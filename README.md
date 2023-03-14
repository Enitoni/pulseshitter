# pulseshitter

[![License](https://img.shields.io/github/license/Enitoni/pulseshitter.svg?style=flat)](https://github.com/Enitoni/pulseshitter/blob/main/LICENSE)

pulseshitter is a shitty solution to a shitty problem.

you see, it all started back in 2015, when discord announced discord, an instant messaging social platform for gamers, until they decided that it was for everyone too.

however, they clearly don't support everyone because they hate us blessed linux users. to this day, discord still [doesn't support](https://support.discord.com/hc/en-us/community/posts/360050971374-Linux-Screen-Share-Sound-Support) sharing audio via screen sharing on linux.

this repository is a follow-up to [pulsecord](https://github.com/itsMapleLeaf/pulsecord) however it shat itself out of existence, and had major issues.

---

- [prerequisites](#prerequisites)
- [usage](#usage)
- [build](#build)
- [contribute](#contribute)
- [license](#license)

---

## prerequisites

- [linux](https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git) (duh)
- [~6 megabytes of ram](https://downloadmoreram.com/)
- [pulseaudio](https://www.freedesktop.org/wiki/Software/PulseAudio/) or [pipewire](https://pipewire.org)
- [parec](https://manpages.debian.org/testing/pulseaudio-utils/parec.1.en.html)
- [discord bot](https://google.com/search?q=discord+bot+token+generator)
- [sanity](https://amnesia.fandom.com/wiki/Sanity)

## usage

[download release](https://github.com/Enitoni/pulseshitter/releases/latest)

```shell
DISCORD_TOKEN='token' DISCORD_USER='user' pulseshitter
```

`token` = your discord bot token

`user` = [your discord account id](https://support.discord.com/hc/en-us/articles/206346498-Where-can-I-find-my-User-Server-Message-ID-)

## build

rust:

- [Mirror 0](https://www.rust-lang.org/)
- [Mirror 1](https://store.steampowered.com/agecheck/app/252490/)
- [Mirror 2](https://en.wikipedia.org/wiki/Rust)

```shell
cargo build
```

## contribute

if you want to???

## license

[MPL-2 license](https://www.mozilla.org/en-US/MPL/2.0/)
