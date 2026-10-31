# Publishing OpenDraft on the AUR

The AUR (Arch User Repository) is a collection of build recipes users install with a helper
such as `yay`. A package there is a git repository holding a `PKGBUILD` (and a `.SRCINFO`
file made from it). The `PKGBUILD` in this folder builds a release from its tag; CI builds it
in an Arch container on every change here (`arch` job in `.github/workflows/packages.yml`).

The AUR's own guide is the authority: <https://wiki.archlinux.org/title/AUR_submission_guidelines>.

## One time

1. **Make an account** at <https://aur.archlinux.org/register> (a username, an email address,
   and an **SSH public key**; if you have none, `ssh-keygen -t ed25519` makes one, and the
   `.pub` file is what you paste in).
2. On an Arch system (or an Arch container: `docker run -it archlinux`), install the tools:
   `sudo pacman -S --needed base-devel git pacman-contrib`.

## Publish the first version

```bash
# 1. An empty repository for the package name (it is created by cloning it).
git clone ssh://aur@aur.archlinux.org/opendraft.git
cd opendraft

# 2. The recipe from this repository. For a release it already has the right pkgver and the
#    sha256sums of that release's source tarball.
cp /path/to/OpenDraft/installer/linux/arch/PKGBUILD .

# 3. Check it builds, and make the file the AUR indexes.
makepkg -si                      # builds and installs it here; try the program
makepkg --printsrcinfo > .SRCINFO

# 4. Publish.
git add PKGBUILD .SRCINFO
git commit -m "opendraft 0.1.4"
git push
```

The package appears at <https://aur.archlinux.org/packages/opendraft> within a minute.

## For each release

1. Make the release (CONTRIBUTING.md, Releasing); the `PKGBUILD` in this repository is updated
   in the same pull request (`pkgver`).
2. Get the new checksum: `updpkgsums` (from `pacman-contrib`) in a folder with the
   `PKGBUILD` downloads the tarball and writes `sha256sums`. Commit that change to this
   repository too.
3. Copy the `PKGBUILD` to the AUR clone, run `makepkg --printsrcinfo > .SRCINFO`, commit
   and push as above.

If a user reports the build fails, the first things to check are a changed tarball checksum
and a new system library; the package `depends` list is in the `PKGBUILD`.
