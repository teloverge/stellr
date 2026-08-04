# Official signing paths for Stellr and LLM Usage Monitor

Date: 2026-08-03

Scope: Stellr's native Windows Tauri/NSIS distribution and `llm-usage-monitor`'s VS Code VSIX extension

Source policy: repository inspection plus Microsoft, Tauri, Visual Studio Code, VSCE, Azure, and GitHub primary sources only

## Executive recommendation

Use two different signing systems because the artifacts have different trust models:

1. **Stellr:** use Windows Authenticode on every executable layer, with a publicly trusted signing identity for production. Prefer **Microsoft Artifact Signing Public Trust** authenticated from GitHub Actions with OpenID Connect (OIDC), integrated through Tauri's `bundle.windows.signCommand` so Tauri can sign the application executable, generated NSIS uninstaller, supporting binaries, and final installer at the correct points in the bundle process. Microsoft recommends Public Trust for publicly shared artifacts, and its managed service keeps certificate lifecycle material in FIPS 140-3 Level 3 HSMs. [Microsoft trust models](https://learn.microsoft.com/en-us/azure/artifact-signing/concept-trust-models); [Artifact Signing overview](https://learn.microsoft.com/en-us/azure/artifact-signing/overview); [Tauri custom signing](https://v2.tauri.app/distribute/sign/windows/#custom-sign-command)
2. **LLM Usage Monitor:** publish through the Visual Studio Marketplace under a real publisher identity. The Marketplace applies its own **repository signature** after upload and VS Code verifies that signature on install. A locally produced VSIX is not automatically Marketplace-signed. Use Microsoft Entra workload identity or VSCE's OIDC publishing support rather than introducing another long-lived publishing token. [VS Code Marketplace signature behavior](https://code.visualstudio.com/docs/configure/extensions/extension-marketplace#_the-extension-signature-cannot-be-verified-by-vs-code); [secure Marketplace publishing](https://code.visualstudio.com/api/working-with-extensions/publishing-extension#_secure-automated-publishing-to-visual-studio-marketplace); [VSCE OIDC option](https://github.com/microsoft/vscode-vsce/blob/main/src/main.ts#L202-L208)

The `@vscode/vsce-sign` dependency found in `llm-usage-monitor` is not an example that can be reused to Authenticode-sign Stellr. VSCE uses it to generate a VSIX signature manifest, assemble a detached signature archive, and verify signatures; an external publisher signing tool must perform the private-key operation. [VSCE signing implementation](https://github.com/microsoft/vscode-vsce/blob/main/src/package.ts#L1799-L1844)

## Keep the four trust statements separate

| Artifact or identity | Signature/verification | What it proves | What it does not prove |
| --- | --- | --- | --- |
| `target\release\stellr.exe` | Windows Authenticode | Publisher identity and integrity of the installed application executable | Integrity of the installer that delivered it |
| `Stellr_*_setup.exe` | Windows Authenticode | Publisher identity and integrity of the outer NSIS installer | That every embedded executable was signed before packaging |
| Marketplace-hosted VSIX | Visual Studio Marketplace repository signature | The package installed by VS Code is the package accepted and signed by the Marketplace | Windows publisher identity for unrelated `.exe` installers |
| Marketplace publisher badge | Domain and Marketplace review | The publisher controls an eligible domain and passed Marketplace review | A cryptographic signature on the VSIX |

Microsoft describes Authenticode as providing authorship and integrity for executable and installable binary formats. A timestamp keeps a signature verifiable after the signing certificate expires. [Microsoft Authenticode timestamping](https://learn.microsoft.com/en-us/windows/win32/seccrypto/time-stamping-authenticode-signatures) The Marketplace explicitly describes its extension signature as a package-integrity and source check performed by VS Code. [VS Code Extension Marketplace](https://code.visualstudio.com/docs/configure/extensions/extension-marketplace#_the-extension-signature-cannot-be-verified-by-vs-code)

## Repository findings

### Stellr

The current Stellr release path is structurally sound but should be modernized:

- `scripts/build-windows-nsis.ps1:21-38` refuses a Release build without a certificate thumbprint and supplies Tauri with `certificateThumbprint`, `digestAlgorithm = sha256`, and a timestamp URL.
- `scripts/build-windows-nsis.ps1:67-72` rejects a release artifact unless PowerShell reports a valid Authenticode signature, but this check covers only the copied outer installer.
- `.github/workflows/release.yml:49-76` stores a Base64 PFX and password as GitHub secrets, imports the certificate into `Cert:\CurrentUser\My`, and builds by thumbprint. The certificate is removed at `.github/workflows/release.yml:92-98`.
- Development artifacts are deliberately unsigned and labeled `UNSIGNED-NOT-FOR-RELEASE`; this is honest and appropriate for local development.

There is one concrete timestamping gap. Stellr sets `timestampUrl` but does not set Tauri's `tsp` option. In Tauri's current signer, `tsp = true` produces RFC 3161 `/tr ... /td ...`; otherwise the same URL is passed with legacy `/t`. [Tauri signer source](https://github.com/tauri-apps/tauri/blob/dev/crates/tauri-bundler/src/bundle/windows/sign.rs#L148-L166) Microsoft recommends RFC 3161 with `/tr` and `/td SHA256` and says Authenticode signatures should always be timestamped. [Microsoft timestamp guidance](https://learn.microsoft.com/en-us/windows/win32/seccrypto/time-stamping-authenticode-signatures#signing-algorithm-recommendations)

Tauri's NSIS bundler also demonstrates why checking only the outer setup executable is insufficient: it prepares a signing command for the generated uninstaller, signs relevant NSIS plugins, and signs the final installer after `makensis` runs. [Tauri NSIS signing source](https://github.com/tauri-apps/tauri/blob/dev/crates/tauri-bundler/src/bundle/windows/nsis/mod.rs#L622-L669) The application executable and outer installer therefore remain distinct verification targets even when Tauri orchestrates both.

### LLM Usage Monitor

The current extension flow packages but does not publisher-sign or publish:

- `D:\dev\llm-usage-monitor\package.json:18` delegates to the extension package task.
- `D:\dev\llm-usage-monitor\apps\vscode-extension\package.json:33` runs `vsce package --no-dependencies --out ../../` without `--sign-tool`.
- `D:\dev\llm-usage-monitor\apps\vscode-extension\package.json:23` still declares the placeholder publisher `local`, which is not a Marketplace publisher identity.
- `D:\dev\llm-usage-monitor\bun.lock:457-477` includes `@vscode/vsce-sign` only as a dependency of `@vscode/vsce` 3.9.2.
- The extension staging script copies JavaScript, web assets, and `tray.ps1`; it does not embed a Windows application executable. There is therefore no inner `.exe` in the current VSIX that needs a separate Authenticode pass.

Plain `vsce package` creates a VSIX. It signs only when the caller explicitly supplies `--sign-tool`; VSCE then invokes that external tool with a generated manifest path and `.p7s` output path. [VSCE CLI option](https://github.com/microsoft/vscode-vsce/blob/main/src/main.ts#L124); [VSCE conditional signing](https://github.com/microsoft/vscode-vsce/blob/main/src/package.ts#L1847-L1858) The presence of `@vscode/vsce-sign` in a lockfile is therefore not evidence that a locally packaged VSIX is signed.

## Stellr: official production path

### Recommended: Artifact Signing Public Trust with OIDC

1. Create an Artifact Signing account, complete public identity validation, and create a **Public Trust** certificate profile. Microsoft supports both organization and individual validation where eligible, and documents the current geographic constraints. [Artifact Signing quickstart](https://learn.microsoft.com/en-us/azure/artifact-signing/quickstart#prerequisites)
2. Grant only the release identity the **Artifact Signing Certificate Profile Signer** role. The official action identifies this role as required. [Azure Artifact Signing action](https://github.com/Azure/artifact-signing-action#authentication)
3. Configure a GitHub Actions federated identity and request `id-token: write`; authenticate with `azure/login`. The official Artifact Signing action recommends OIDC and shows this permission model. [Azure Artifact Signing action OIDC example](https://github.com/Azure/artifact-signing-action#example) GitHub's own guidance explains that OIDC avoids storing long-lived cloud credentials in GitHub secrets. [GitHub OIDC security guidance](https://docs.github.com/en/actions/how-tos/secure-your-work/security-harden-deployments)
4. Integrate the Microsoft SignTool + Artifact Signing dlib command behind a small Tauri `signCommand` wrapper. Tauri substitutes each path at `%1`, including files that must be signed during NSIS construction. [Tauri `signCommand`](https://v2.tauri.app/distribute/sign/windows/#custom-sign-command) Microsoft's supported SignTool integration uses `/dlib`, `/dmdf`, `/fd SHA256`, `/tr`, and `/td SHA256`. [Microsoft SignTool integration](https://learn.microsoft.com/en-us/azure/artifact-signing/how-to-signing-integrations#use-signtool-to-sign-a-file)
5. Use the Artifact Signing timestamp service, `http://timestamp.acs.microsoft.com`, with SHA-256. Artifact Signing certificates are short-lived, so Microsoft calls timestamping critical. [Microsoft Artifact Signing integration](https://learn.microsoft.com/en-us/azure/artifact-signing/how-to-signing-integrations#use-signtool-to-sign-a-file)

This approach avoids storing an exportable production private key or PFX password in GitHub. Artifact Signing performs digest signing and keeps certificate lifecycle material in managed HSMs; the file itself remains on the runner. [Artifact Signing overview](https://learn.microsoft.com/en-us/azure/artifact-signing/overview#features)

Do not merely run a signing action against the finished setup executable. That would sign the outer installer but leave any previously packaged inner executable or generated uninstaller unchanged. The signer must participate before the inner files are embedded and again after the outer NSIS executable is produced; Tauri's `signCommand` is the appropriate orchestration seam. [Tauri NSIS signing sequence](https://github.com/tauri-apps/tauri/blob/dev/crates/tauri-bundler/src/bundle/windows/nsis/mod.rs#L622-L669)

### Acceptable alternative: public CA certificate or managed key vault

A code-signing certificate chaining to a CA in the Microsoft Trusted Root Program is also a valid public-distribution path. Microsoft says public trust is what Windows security features such as Smart App Control consume when cloud reputation is unavailable. [Smart App Control overview](https://learn.microsoft.com/en-us/windows/apps/develop/smart-app-control/overview) Use the issuer's supported hardware or cloud-key integration and Tauri's custom signer when the key is not in the local Windows certificate store.

Stellr's current Base64-PFX import can remain as a compatibility path only when the issuer actually supplies an exportable PFX and permits that storage model. Tauri explicitly warns that its documented PFX/OV walkthrough applies only to OV certificates acquired before 2023-06-01 and directs newer OV/EV customers to their issuer's instructions or a custom signing command. [Tauri Windows signing warning](https://v2.tauri.app/distribute/sign/windows/#ov-certificates)

If the PFX path is retained temporarily, add `tsp = true` to the temporary Tauri signing configuration, keep the PFX/password in protected GitHub environment secrets, import only for the release job, remove both certificate-store entry and temporary PFX in an `always()` cleanup, and fail closed when any signature or timestamp check fails. Tauri documents GitHub encrypted-secret storage for its PFX flow. [Tauri GitHub Actions signing](https://v2.tauri.app/distribute/sign/windows/#sign-your-application-with-github-actions)

### Required production verification

Run verification against at least these independent artifacts:

```powershell
signtool verify /pa /all /v target\release\stellr.exe
signtool verify /pa /all /v artifacts\windows-x64\Stellr_*_windows-x64_nsis.exe
```

After a clean install, run the same verification against the installed `stellr.exe` and the installed uninstaller, then run the existing launch/uninstall smoke test. SignTool's `/pa` selects the default Authenticode policy, `/all` verifies every embedded signature, and `/tw` can additionally warn when a timestamp is missing. [Microsoft SignTool verification options](https://learn.microsoft.com/en-us/windows/win32/seccrypto/signtool#verify-command-options)

`Get-AuthenticodeSignature` remains a useful PowerShell assertion, but the release evidence should retain SignTool's verbose certificate-chain, digest, and timestamp output for both inner and outer targets. A release should fail on SignTool exit code 1 or 2; Microsoft defines 2 as "completed with warnings," which is too weak for a fail-closed signing gate. [SignTool return values](https://learn.microsoft.com/en-us/windows/win32/seccrypto/signtool#return-value)

## Stellr: safe local-development path

The default local build should remain unsigned and clearly named `UNSIGNED-NOT-FOR-RELEASE`. Signing every debug build with the production identity increases exposure and consumes a production trust signal without helping ordinary development.

For testing only, use either:

- an Artifact Signing **Public Trust Test** profile, which Microsoft explicitly says is not publicly trusted and is intended for inner-loop development/testing; or
- a self-signed RSA code-signing certificate created with `New-SelfSignedCertificate`, trusted only on a designated test machine.

[Artifact Signing test trust](https://learn.microsoft.com/en-us/azure/artifact-signing/concept-trust-models#public-trust-model); [Microsoft self-signed testing guidance](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_signing#methods-of-signing-scripts)

A self-signed certificate tests mechanics--selection, signing order, installation, and verification under a test trust root--but it does not create public Windows trust or SmartScreen reputation. Never attach such an artifact to a public release, and never install the test root on user machines. Microsoft states that self-signed certificates are for testing and are not appropriate for shared software. [Microsoft `about_Signing`](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_signing#methods-of-signing-scripts)

## LLM Usage Monitor: official production path

### Marketplace repository signing is the baseline

1. Create the intended Visual Studio Marketplace publisher and replace `"publisher": "local"` with its immutable publisher ID. Every extension manifest requires a publisher identity. [VS Code publisher setup](https://code.visualstudio.com/api/working-with-extensions/publishing-extension#_create-a-publisher)
2. Build and test the extension, then publish with current `@vscode/vsce` rather than treating a locally generated `.vsix` release asset as the canonical distribution.
3. Prefer Microsoft Entra workload identity federation. VS Code's current publishing guide recommends identity-based automated publishing to eliminate long-lived PATs and notes retirement of global Azure DevOps PATs on 2026-12-01. [secure automated publishing](https://code.visualstudio.com/api/working-with-extensions/publishing-extension#_secure-automated-publishing-to-visual-studio-marketplace)
4. Let the Marketplace apply its repository signature. VS Code verifies that signature on install and update. [Marketplace repository signing](https://code.visualstudio.com/docs/configure/extensions/extension-marketplace#_the-extension-signature-cannot-be-verified-by-vs-code)

Current VSCE source also exposes `vsce publish --oidc` as "OpenID Connect trusted publishing" and `--azure-credential` for Microsoft Entra authentication. [VSCE publishing options](https://github.com/microsoft/vscode-vsce/blob/main/src/main.ts#L195-L223) Use the Marketplace's documented publisher authorization setup for the selected identity; do not repurpose `GITHUB_TOKEN`, which is not a Visual Studio Marketplace publishing credential.

### Publisher signing is separate and optional

Current VSCE can accept publisher-signing material in three ways: an external `--sign-tool`, manifest plus `.p7s`, or a prebuilt signature archive. [VSCE publish options](https://github.com/microsoft/vscode-vsce/blob/main/src/main.ts#L219-L223) With `--sign-tool`, VSCE:

1. generates a signature manifest from the VSIX;
2. invokes the external tool with the manifest input and `.signature.p7s` output;
3. packages the manifest and signature into a detached signature archive; and
4. uploads that signature alongside the VSIX when publishing.

[VSCE signing source](https://github.com/microsoft/vscode-vsce/blob/main/src/package.ts#L1799-L1844)

This publisher-signature protocol is not the same operation as embedding Authenticode into a Windows PE executable. Do not point Stellr's Authenticode command at a VSIX, and do not assume `@vscode/vsce-sign` owns or uses a publisher private key. It is the format/manifest/verification helper; the publisher supplies the actual signing tool.

The official public VS Code publishing guide does not currently make publisher signing a prerequisite for Marketplace publication. Therefore the production baseline for this repository should be Marketplace publication plus repository signing. Add publisher signing only after the Marketplace publisher account has a documented certificate policy and the chosen external signer can be operated in CI without exporting a long-lived private key.

### Publisher verification is not package signing

Apply for the Marketplace verified-publisher badge when eligible. It validates control of an eligible HTTPS domain and includes Marketplace review; current prerequisites require both the publisher's extension history and domain registration to be at least six months old. [VS Code verified publisher requirements](https://code.visualstudio.com/api/working-with-extensions/publishing-extension#_verify-a-publisher) This improves identity signaling but does not replace repository or publisher cryptographic signatures.

## LLM Usage Monitor: safe local-development path

Keep `vsce package --no-dependencies` for reproducible side-loading and install the resulting VSIX only on development/test VS Code profiles. VS Code supports local VSIX installation, and auto-update is disabled by default for a directly installed VSIX. [VS Code local VSIX installation](https://code.visualstudio.com/docs/configure/extensions/extension-marketplace#_install-from-a-vsix)

Do not claim the local VSIX is signed merely because `@vscode/vsce-sign` appears in `bun.lock`. If an offline acceptance test requires signature evidence, download the published package through VS Code/Marketplace so the Marketplace repository signature can be verified, or deliberately implement and verify the external publisher-signing flow. VSCE exposes `verify-signature` for a VSIX plus manifest and `.p7s`. [VSCE verification command](https://github.com/microsoft/vscode-vsce/blob/main/src/main.ts#L345-L355)

## Recommended implementation order

1. **Immediate Stellr correction:** enable RFC 3161 timestamping (`tsp = true`) and verify both the release application executable and the final NSIS installer with `signtool verify /pa /all /v`.
2. **Production Stellr identity:** provision Artifact Signing Public Trust, establish GitHub OIDC federation and least-privilege signing RBAC, and replace the exportable-PFX default with a Tauri `signCommand` wrapper over Microsoft's supported SignTool integration.
3. **Signing coverage gate:** verify the built app, outer installer, clean-installed app, and installed uninstaller; retain verbose signature evidence with the release artifacts.
4. **VSIX publishing identity:** create the real Marketplace publisher, update the manifest publisher ID, and add identity-federated `vsce publish` automation.
5. **VSIX trust:** rely on Marketplace repository signing as the production baseline. Treat publisher signing as a separate hardening project, not as a prerequisite and not as a substitute for Windows Authenticode.

## Primary sources

- [Microsoft SignTool](https://learn.microsoft.com/en-us/windows/win32/seccrypto/signtool)
- [Microsoft Authenticode timestamping](https://learn.microsoft.com/en-us/windows/win32/seccrypto/time-stamping-authenticode-signatures)
- [Microsoft Artifact Signing overview](https://learn.microsoft.com/en-us/azure/artifact-signing/overview)
- [Microsoft Artifact Signing trust models](https://learn.microsoft.com/en-us/azure/artifact-signing/concept-trust-models)
- [Microsoft Artifact Signing integrations](https://learn.microsoft.com/en-us/azure/artifact-signing/how-to-signing-integrations)
- [Azure Artifact Signing GitHub Action](https://github.com/Azure/artifact-signing-action)
- [Tauri Windows code signing](https://v2.tauri.app/distribute/sign/windows/)
- [Tauri Windows signing source](https://github.com/tauri-apps/tauri/blob/dev/crates/tauri-bundler/src/bundle/windows/sign.rs)
- [Tauri NSIS bundler signing source](https://github.com/tauri-apps/tauri/blob/dev/crates/tauri-bundler/src/bundle/windows/nsis/mod.rs)
- [VS Code extension publishing](https://code.visualstudio.com/api/working-with-extensions/publishing-extension)
- [VS Code Extension Marketplace signature behavior](https://code.visualstudio.com/docs/configure/extensions/extension-marketplace)
- [Microsoft VSCE source](https://github.com/microsoft/vscode-vsce)
