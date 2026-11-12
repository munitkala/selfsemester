# Contributing to SelfSemester

Thanks for taking a look. Bug reports, ideas and fixes are all welcome, and you don't need to write code to help:
telling us that a lesson was confusing or a mark looked wrong is just as useful.

## Reporting a problem or suggesting something

Open an [issue](https://github.com/munitkala/selfsemester/issues/new/choose) and pick the template that fits. For
problems, the version (Settings, at the bottom of **Your data and backups**) and the steps that led to it help most.
Please don't paste anything private: your submitted work, your Claude account details or other people's names.

If something in a generated course looks copied from a book or website, or a figure looks like someone else's,
report it the same way. See [DISCLAIMER.md](DISCLAIMER.md) for how course content is handled.

## Working on the code

You need Node.js 22.17+, Rust 1.85+ and the Visual Studio C++ build tools on Windows.

```powershell
npm install
npm run tauri dev     # run the app with hot reload
```

Before opening a pull request, run the same checks the build does:

```powershell
npm run check         # type-check the interface
npm run build         # build the interface
cd src-tauri
cargo test --locked   # unit tests, including a simulated course from start to transcript
```

To try the app without touching your real courses, set `SELFSEMESTER_HOME` to an empty folder, and set
`SELFSEMESTER_NO_CLAUDE=1` if you don't want any Claude calls. The [README](README.md#development) has more on the
layout of the code.

## A few house rules

- **Keep it plain.** Interface text and docs use short, everyday words. Explain why something matters, not just
  what it does.
- **Nothing copied.** Code, icons, figures and text must be your own or under a licence compatible with MIT, and any
  new library needs an entry in [THIRD_PARTY.md](THIRD_PARTY.md) with its licence. No GPL or AGPL libraries in the app.
- **Privacy first.** Everything stays on the learner's computer. Don't add analytics, accounts or network calls
  beyond Claude Code.
- **Themes and motion.** Use the colour variables in `src/app.css` rather than fixed colours, so every theme works,
  and keep animation optional under the motion and effects settings.
- **Add to the changelog.** Note user-visible changes under **Unreleased** in [CHANGELOG.md](CHANGELOG.md).

By contributing you agree that your work is released under the [MIT licence](LICENSE).
