# Contribution Instructions

Found a bug? Missing a feature? Docs unclear? You can open an issue, or
directly send a PR for small changes!

## General Notes

You can view the Bluetooth specifications on [this website][bt-specs]. At the
time of writing, we're trying to implement and comply with Bluetooth 4.2.

## Filing Issues

Issues should generally be opened when:

* You found a bug and don't know how to fix it or what causes it
* You need a complicated feature that requires API design, a larger
  refactoring, or is just too massive to do it all at once
* You don't know how to do what you want to do, or don't have time to do it

If none of these applies, you can just directly send a PR if you want.

## What should I work on?

Interested in helping out, but don't know what to do? Here's some guidance.
What are you interested in?

* Like doing technical writing? Like ASCII art? Documentation can always be
  improved!

  Protocol implementations (like L2CAP or ATT) should include a description of
  the protocol (including simple diagrams or the involved packets) and all
  modules should ideally give a high-level overview of the contained
  functionality. The specification is mostly awful, so we basically have to do
  this ourselves.

  Specific issues that need documentation work are labeled with
  `status: needs docs`, but feel free to work on something that isn't listed
  there!

* Like thinking about how systems interact with each other? There's lots of
  tricky design work to be done!

  Due to the nature of Bluetooth, we have to split the stack into a real-time
  part (performing connection maintenance, acknowledgement and channel hopping)
  and a non-real-time part that does most of the request/response processing.
  This makes for interesting design constraints.

  Issues involving this kind of systems design are labeled with
  `status: needs design`.

* Do you like designing Rust APIs? We've got a pretty large API surface to
  cover, and being entirely `#![no_std]` and allocationless imposes some
  interesting constraints.

  Find issues in need of API design by looking for the `status: needs design`
  label.

* Just want to hack on something already? We've got you covered. The spec is
  large enough for all of us.

  Issues in need of implementation or other coding work are labeled as
  `status: needs code`.

## Working on the code

We try to extensively document all the code, so you can check out the [hosted
API docs] or jump straight in.

### Code Style

Generally: Do what's already done in existing files. More specifically, that
includes:

* Use `rustfmt` to format code. This is checked in CI. It is recommended that
  you set up your IDE to automatically run `rustfmt` on save for working on
  Rubble.
* Maximum line length is 100 columns. Documentation should also wrap after 100
  columns to save vertical space.
* All `use` imports should be grouped into a single one (instead of adding a
  `use` per imported crate or module).

[bt-specs]: https://www.bluetooth.com/specifications/archived-specifications
[hosted API docs]: https://jonas-schievink.github.io/rubble/
