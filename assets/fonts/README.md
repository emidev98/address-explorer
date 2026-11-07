# Bundled typefaces

These fonts are bundled so the application renders the same typography on
every platform and does not request a remote font.

| File | Family | Axes / weights | Source |
| --- | --- | --- | --- |
| `roboto-variable.woff2` | Roboto | `wght` | [`@fontsource-variable/roboto`](https://fontsource.org/fonts/roboto) 5.3.0, Latin subset |
| `fraunces-variable.woff2` | Fraunces | `opsz`, `wght`, `SOFT`, `WONK` | [`@fontsource-variable/fraunces`](https://fontsource.org/fonts/fraunces) 5.3.0, Latin subset |
| `inter-variable.woff2` | Inter | `wght` | [`@fontsource-variable/inter`](https://fontsource.org/fonts/inter) 5.3.0, Latin subset |
| `ibm-plex-mono-400.woff2` | IBM Plex Mono Regular | 400 | [`@fontsource/ibm-plex-mono`](https://fontsource.org/fonts/ibm-plex-mono) 5.3.0, Latin subset |
| `ibm-plex-mono-500.woff2` | IBM Plex Mono Medium | 500 | [`@fontsource/ibm-plex-mono`](https://fontsource.org/fonts/ibm-plex-mono) 5.3.0, Latin subset |

The files are injected through the `FontFaces` component in
`src/components/theme.rs`, which uses `asset!` URLs so Dioxus can serve the
content-hashed files on web and desktop.

All four families are licensed under the SIL Open Font License 1.1. The
license texts are checked in next to the fonts as `OFL-Roboto.txt`,
`OFL-Fraunces.txt`, `OFL-Inter.txt`, and `OFL-IBMPlexMono.txt`.
