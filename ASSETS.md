# Assets attribution

The files under `assets/original/` are **not** covered by this project's MIT license.

## Origin

Icons in `assets/original/textures/`, sounds in `assets/original/sounds/`, and emote
animations / icons / sounds in `assets/original/emotes/` come from a local
League of Legends install (same set shipped by
[darrenprx/lolPing](https://github.com/darrenprx/lolPing)).

They are **© Riot Games**. This project does not claim ownership of them and
does not license them under MIT (or any other open-source license).

## Quality / packaging policy

- **Canonical runtime assets** stay as the original animated WebP (+ PNG
  wheel icons + OGG/WAV sounds). Do not replace originals with lossy WebM.
- Optional derived formats (e.g. VP9 WebM at CRF ≈ 10 for experiments) may
  live only under `assets/derived/` (gitignored by default) and must never
  overwrite `assets/original/`.
- Release packaging should prefer **on-demand loading** of heavy animations
  so the install footprint stays tiny while quality stays at original WebP.

## Contact

If you represent Riot Games and want something removed, please open an issue.

lolPing-lite is not endorsed by Riot Games and does not reflect the views or
opinions of Riot Games or anyone officially involved in producing or managing
Riot Games properties. Riot Games and all associated properties are trademarks
or registered trademarks of Riot Games, Inc.
