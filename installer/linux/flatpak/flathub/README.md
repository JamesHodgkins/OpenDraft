# Submitting OpenDraft to Flathub

These are the files for a Flathub submission of the release named in the manifest
(`uk.openengineering.opendraft.yml`: the git tag and commit under `sources`) with the
crate list made from that tag's `Cargo.lock` (`cargo-sources.json`). CI builds them
exactly as Flathub will (`flathub` job in `.github/workflows/packages.yml`).

Flathub's own instructions are the authority and change from time to time; read
<https://docs.flathub.org/docs/for-app-authors/submission> alongside this. What follows is
the path as it was understood when this was written.

## Before you start

- A GitHub account with two-factor authentication on (Flathub requires it for maintainers).
- You have signed in to <https://flathub.org> with GitHub.
- The release (tag) exists and CI is green for these files.

## Steps

1. **Fork** <https://github.com/flathub/flathub> to your account. Untick "Copy the `master`
   branch only": you need the `new-pr` branch.
2. **Clone your fork on the `new-pr` branch** and make a branch for the submission:

   ```bash
   git clone --branch=new-pr https://github.com/<your-user>/flathub.git
   cd flathub
   git checkout -b add-opendraft new-pr
   ```

3. **Copy in the two files** from this folder, into the root of that clone (no sub-folder):
   `uk.openengineering.opendraft.yml` and `cargo-sources.json`.
4. **Commit and push:**

   ```bash
   git add uk.openengineering.opendraft.yml cargo-sources.json
   git commit -m "Add uk.openengineering.opendraft"
   git push origin add-opendraft
   ```

5. **Open a pull request** from `add-opendraft` on your fork **to `flathub/flathub`, base
   branch `new-pr`** (not `master`). Title: `Add uk.openengineering.opendraft`. Use the text
   in `PULL_REQUEST.md` beside this file for the description, and tick the checklist the
   template shows.
6. **Wait for the build and the reviewers.** A bot builds the app and reports; a volunteer
   reviews. Expect questions or requests for changes. Answer on the pull request and push
   fixes to the same branch. Comment `bot, build` if you are told to ask for a rebuild.
7. **When it is merged**, Flathub creates `flathub/uk.openengineering.opendraft` and invites
   you to it. Accept the invitation (an email, or the notifications page on GitHub). From
   then on, updates are pull requests to that repository.
8. **Verify the domain** (see `../README.md`): in the Flathub developer portal, open the app and
   follow the verification steps for `openengineering.uk`. The app is installable before
   this; it just shows as unverified.

## For the next release

Make the release as usual (CONTRIBUTING.md, Releasing), then in the Flathub repository change
`tag` and `commit` under the first source, regenerate `cargo-sources.json` from the new
`Cargo.lock` (steps in `../README.md`), and open a pull request there. Keep the copy of the
manifest in this folder in step so CI keeps testing what is submitted.
