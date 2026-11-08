# Setting up GitHub releases and SignPath code signing

Do these once. After that, every release is one command:
`powershell -ExecutionPolicy Bypass -File .\release.ps1 -Publish`

## 1. Install Git and set your name

1. Install Git for Windows from <https://git-scm.com/download/win> (the default options are fine),
   then close and reopen PowerShell.
2. Tell Git who you are. Commits are public, so use your GitHub username and GitHub's private
   "noreply" email (GitHub → Settings → Emails → turn on **Keep my email addresses private**,
   then copy the `...@users.noreply.github.com` address shown there):

   ```powershell
   git config --global user.name  "pcrawford576"
   git config --global user.email "12345678+pcrawford576@users.noreply.github.com"
   ```

## 2. Create the public repository and upload the code (GitHub Desktop)

1. Move the GitHub build file into place (GitHub only looks in `.github\workflows`):

   ```powershell
   cd "C:\Users\Pat\Documents\GitHub\Respawn The Gamers Browser"
   New-Item -ItemType Directory -Force .github\workflows | Out-Null
   Move-Item docs\release-workflow.yml .github\workflows\release.yml
   ```

2. In GitHub Desktop, check the **Changes** list. It must **not** include `src-tauri/target`,
   `.idea`, `android/local.properties`, `android/app/release` or any `.jks`/`.keystore` file
   (the `.gitignore` file keeps those out). It should include `.github/workflows/release.yml`.
3. Type a summary such as `Respawn 0.1.2` and click **Commit to main**.
4. Click **Publish repository**. **Untick "Keep this code private"** (SignPath only signs
   public projects), then publish.

`release.ps1 -Publish` uses the `git` command. GitHub Desktop's own copy of Git isn't
available in PowerShell, so also install Git for Windows (step 1) before using `-Publish`.

## 3. Turn on two-factor sign-in on GitHub

GitHub → Settings → **Password and authentication** → **Enable two-factor authentication**.
SignPath requires this.

## 4. Publish a first (unsigned) release

SignPath only accepts projects that already have a release out.

```powershell
powershell -ExecutionPolicy Bypass -File .\release.ps1 -Publish
```

Watch it build under the repo's **Actions** tab (about 10–15 minutes the first time). The
installer then appears under **Releases**, marked "not code signed yet".

## 5. Apply to SignPath Foundation

Go to <https://signpath.org/apply> and fill in the form:

- Project name: **Respawn**
- Repository: `https://github.com/pcrawford576/Respawn-The-Gamers-Browser`
- License: **MIT**
- Download page: `https://github.com/pcrawford576/Respawn-The-Gamers-Browser/releases`
- Code signing policy: `https://github.com/pcrawford576/Respawn-The-Gamers-Browser/blob/main/CODE_SIGNING_POLICY.md`
- Build system: **GitHub Actions**

Approval usually takes a few days. You'll get a SignPath account; turn on two-factor
sign-in there too.

## 6. Connect SignPath to GitHub (after approval)

1. Install the **SignPath GitHub App** (<https://github.com/apps/signpath>) and give it access
   to the `respawn` repository only.
2. In SignPath, open the project (its "slug" is probably `respawn`):
   - **Trusted build systems**: make sure GitHub.com is linked to the project.
   - **Artifact configurations → default**: paste the contents of
     `.signpath/artifact-configuration.xml` and save.
   - **Signing policies**: note the slug of the release policy (usually `release-signing`).
3. In SignPath, create an **API token** (user menu → API tokens), and give that user the
   **Submitter** role on the release signing policy.
4. In GitHub: repo → **Settings → Secrets and variables → Actions**:
   - **Secrets** tab → New secret `SIGNPATH_API_TOKEN` = the API token.
   - **Variables** tab → New variable `SIGNPATH_ORGANIZATION_ID` = your organization ID
     (the long ID in the SignPath web address after `/Web/`).
   - Only if they differ from the defaults: `SIGNPATH_PROJECT_SLUG` (default `respawn`) and
     `SIGNPATH_SIGNING_POLICY_SLUG` (default `release-signing`).

## 7. Release signed builds

```powershell
powershell -ExecutionPolicy Bypass -File .\release.ps1 -Publish
```

GitHub builds Respawn, then sends **two** signing requests to SignPath: first the app
(`respawn.exe`), then the installer. Approve each one in SignPath (you'll get an email);
the build waits up to an hour for each approval. The signed installer then appears under
**Releases**.

Windows SmartScreen may still warn on the first downloads of a new signed version until it
has seen enough of them; this fades as more people install it.
