# Credits for the demo recordings

Each scene is clean speech mixed with everyday noise, then processed by the two Hush AI engines.

## Speech

The clean speech is from the VoiceBank-DEMAND clean test set (48 kHz): speakers p232 (male) and
p257 (female). These recordings come from the VCTK corpus.

- Valentini-Botinhao, Cassia (2017). *Noisy speech database for training speech enhancement
  algorithms and TTS models.* University of Edinburgh, School of Informatics, Centre for Speech
  Technology Research (CSTR). https://doi.org/10.7488/ds/2117. License: CC BY 4.0
  (https://creativecommons.org/licenses/by/4.0/).
- Yamagishi, Junichi; Veaux, Christophe; MacDonald, Kirsten (2019). *CSTR VCTK Corpus: English
  Multi-speaker Corpus for CSTR Voice Cloning Toolkit (version 0.92).* University of Edinburgh, CSTR.
  https://doi.org/10.7488/ds/2645. License: CC BY 4.0.

Changes: sentences are joined with short pauses, and noise is added.

## Noise

All noise clips are from Freesound and are released under Creative Commons 0 (public domain,
https://creativecommons.org/publicdomain/zero/1.0/). The license was checked on each sound's own
page. The clips were taken from the Freesound MP3 previews, cut, mixed and changed in level.

| Scene | Sound ID | Title | Author | URL | License |
| --- | ---: | --- | --- | --- | --- |
| Dog barking (`dog`) | 635877 | Small dog barking.wav | Wax_vibe | https://freesound.org/people/Wax_vibe/sounds/635877/ | Creative Commons 0 |
| Keyboard and mouse (`office`) | 622084 | Mechanical keyboard typing | Shyinka | https://freesound.org/people/Shyinka/sounds/622084/ | Creative Commons 0 |
| Keyboard and mouse (`office`) | 146877 | Mouse clicks and scroll wheel | elle-trudgett | https://freesound.org/people/elle-trudgett/sounds/146877/ | Creative Commons 0 |
| Keyboard and mouse (`office`) | 636861 | Office chatter and typing.wav | Alex_hears_things | https://freesound.org/people/Alex_hears_things/sounds/636861/ | Creative Commons 0 |
| Chatter and dishes (`cafe`) | 616602 | 120710 Diner, small restaurant, voices, cash register, dishes, voices, Blyth.wav | TRP | https://freesound.org/people/TRP/sounds/616602/ | Creative Commons 0 |
| Traffic and siren (`street`) | 527697 | Tel Aviv Israel street ambience - traffic and ambulance sirens in background | khenshom | https://freesound.org/people/khenshom/sounds/527697/ | Creative Commons 0 |
| Hammering and banging (`construction`) | 650696 | Construction site with hammering and banging noises | khenshom | https://freesound.org/people/khenshom/sounds/650696/ | Creative Commons 0 |
| Crying baby (`baby`) | 151080 | 01 baby crying hard and long.wav | PickleJones | https://freesound.org/people/PickleJones/sounds/151080/ | Creative Commons 0 |

## Processing

Each enhanced file is raw engine output with one shared gain per scene: the noisy file and both
enhanced files of a scene get the same gain. No EQ, gating or other processing is applied.

- `*-noisy.mp3`: the mix that both engines received.
- The app runs as one continuous stream. So each engine first ran on 5 s of the same noise recording
  (the part just before the excerpt), and these 5 s are cut from all three files.
- `*-dfn3.mp3`: DeepFilterNet 3, official `deep-filter` 0.5.6 binary, default settings, 48 kHz,
  streaming. Its 30 ms delay is removed so that the files line up.
- `*-reuse.mp3`: NVIDIA Real-time RE-USE run by the Hush engine (`hush-core`), 48 kHz, 3 layers,
  no look-ahead (the app's Studio quality), streaming in 1000-sample chunks. The RE-USE weights are
  NVIDIA material under the NVIDIA One-Way Noncommercial License (see `../LICENSE`).

The metrics in `../demos.json` (PESQ-WB, STOI and SI-SDR at 16 kHz) compare each file with the clean
speech before it was mixed.
