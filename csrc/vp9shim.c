// Tiny C shim over libvpx's VP9 decoder so Rust doesn't depend on libvpx struct layouts.
#include <stdlib.h>
#include <stdint.h>
#include <vpx/vpx_decoder.h>
#include <vpx/vp8dx.h>

void *lp_vp9_new(int threads) {
  vpx_codec_ctx_t *c = calloc(1, sizeof *c);
  if (!c) return NULL;
  vpx_codec_dec_cfg_t cfg = {0};
  cfg.threads = threads > 0 ? threads : 1;
  if (vpx_codec_dec_init(c, vpx_codec_vp9_dx(), &cfg, 0) != VPX_CODEC_OK) { free(c); return NULL; }
  return c;
}

// Returns 1 with a frame, 0 no frame, <0 error. Plane pointers valid until next decode.
int lp_vp9_decode(void *h, const uint8_t *data, size_t len, const uint8_t **planes,
                  int *strides, int *w, int *hh) {
  vpx_codec_ctx_t *c = h;
  if (vpx_codec_decode(c, data, (unsigned int)len, NULL, 0) != VPX_CODEC_OK) return -1;
  vpx_codec_iter_t it = NULL;
  vpx_image_t *img = vpx_codec_get_frame(c, &it);
  if (!img) return 0;
  if (img->fmt != VPX_IMG_FMT_I420) return -2;
  for (int i = 0; i < 3; i++) { planes[i] = img->planes[i]; strides[i] = img->stride[i]; }
  *w = (int)img->d_w; *hh = (int)img->d_h;
  return 1;
}

void lp_vp9_free(void *h) {
  if (!h) return;
  vpx_codec_destroy(h);
  free(h);
}
