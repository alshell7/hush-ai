# Contributing

Thank you for helping make Hush AI better.

## Ways to help

- **Try it** on your computer and [report what breaks](https://github.com/alshell7/hush-ai/issues/new/choose):
  system, microphone, meeting app and what you heard.
- **Pick a roadmap item** from the [README](README.md#roadmap) and open an issue to talk
  about the approach first.
- **Improve the docs**: short, plain sentences; one idea per sentence.

## Before you send a pull request

1. Read [Build and release](docs/BUILD.md) and run the tests.
2. Keep audio rules: no allocation, locks or waiting in audio callbacks; blocking device
   calls stay on the control thread.
3. Keep the regression gates. Never loosen a gate to make a change pass.
4. Measure performance changes with release builds, and nothing else running.
5. Do not commit recordings of anyone's voice, secrets, tokens or model weights.
6. Match the style of the code around you. `cargo fmt` and `npm run typecheck` must pass.

## Design

The app's look is warm white with an opal palette (peach, apricot, mint, aqua), a deep ink
for text and primary buttons, Geist type and big rounded cards. Tokens are in
[`app/src/styles.css`](app/src/styles.css). Motion is short and springy, and nothing
animates while the window is hidden.

## License

By contributing you agree that your contribution is licensed under Apache-2.0.
