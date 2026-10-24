<div align="center">

[العربية](README.md) | **English**

</div>

<img src="assets/brand/logo-full.svg" alt="SAYL" width="220">

# SAYL Download Manager

[![CI](https://github.com/athar-studios/sayl/actions/workflows/ci.yml/badge.svg)](https://github.com/athar-studios/sayl/actions)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

<!-- [Screenshot placeholder: Application UI screenshot will be added here once official screenshots are published to docs/screenshots/v0.1.0/] -->

**SAYL** is an independent, high-performance download manager for Windows designed for speed, reliable resuming, and complete user privacy. It delivers fast multi-part downloads, resilient state persistence across crashes, and a modern clean interface with zero telemetry and zero ads.

---

### What is SAYL?
SAYL manages and accelerates file downloads over standard HTTP and HTTPS protocols. It splits files into parallel byte ranges to maximize connection bandwidth, continuously validates received chunks, and ensures that interrupted transfers resume accurately without data corruption.

Key highlights:
- **Verified Resuming:** Strict byte-range validation and strong ETag checking prevent corrupted resumes when servers or files change (provided the remote server supports range requests and the file has not been modified).
- **Crash Resilience:** Durable transaction-safe journaling ensures unfinished tasks recover automatically after sudden reboots or power loss.
- **Absolute Privacy:** No tracking, no analytics SDKs, no ads, and no external telemetry calls. Your download history and URLs stay local on your machine.

---

### Download & Installation
Download the latest official release for Windows directly from GitHub:  
👉 **[Download SAYL — Latest Release](https://github.com/athar-studios/sayl/releases/latest)**

#### Available Packages
- **Standard NSIS Installer (Recommended):** `SAYL-Setup-x64-0.1.0.exe`  
  Download and run the setup executable to install SAYL on your PC.
- **Windows Installer Package (MSI):** `SAYL-Setup-x64-0.1.0.msi`  
  Alternative MSI package suitable for silent installation or enterprise deployment.

#### Verifying Package Integrity
Each release publishes an accompanying `SHA256SUMS.txt` file. You can verify your installer using PowerShell:
```powershell
Get-FileHash .\SAYL-Setup-x64-0.1.0.exe -Algorithm SHA256
```
Compare the output hash against the one published in `SHA256SUMS.txt`.

---

### Starting Your First Download

#### Adding a Download
1. Click **"Add Download"** in the toolbar or press `Ctrl+N`.
2. Paste the target file URL into the dialog.
3. Click **"Start Download"** to initiate the transfer.

#### Handling Duplicate URLs
When adding a URL that already exists in the downloads list, SAYL prompts with a dialog offering three clear actions:
- **Open existing:** Switch directly to the existing download task and view its details.
- **Download again:** Start a new, independent download task for the same URL.
- **Cancel:** Close the dialog and dismiss the addition.

#### Drag and Drop
Drag any download link directly from your browser or desktop application and drop it into the SAYL window to begin downloading immediately.

#### Categories & Collapsible Sidebar
SAYL features a collapsible sidebar that organizes your downloads by file category with live task counts:
- **Available Categories:** All, Video, Audio, Documents, Programs, Compressed, and Other.
- **Automatic Classification:** Files are categorized based on their file extension first, falling back to the HTTP MIME content type (`Content-Type`) when extensions are absent or ambiguous.

#### Search & State Filtering
- **Quick Search:** Real-time search box filtering downloads by matching file name or URL.
- **State Filter:** Quick filter chips to view tasks by status: All, Active (Running), Paused, Completed, and Failed.

#### View Modes: Cards & Compact Table
SAYL offers two view layouts to match your preference:
- **Card View (Default):** Rich visual cards displaying progress, size, and download speed.
- **Compact Table View:** Streamlined tabular layout displaying Name, Size, Progress, Speed, Remaining Time, State, and quick Action buttons.
- Your preferred view mode is saved automatically in Settings and restored across sessions.

#### Floating Download Window
A small floating panel inside the app, one per active download, overlays the task list so progress can be watched without leaving the current view:
- **Metrics Displayed:** File name, size, gradient progress bar, real-time speed, estimated remaining time, and active segment count (`Parts`).
- **Direct Controls:** Pause, resume, and cancel buttons.
- **Completion Actions:** Offers **"Open file"** and **"Show in folder"** buttons once the transfer completes.
- **Configuration:** Governed by two preferences in Settings: "Show download window when download starts" (on by default) and "Close window automatically when finished" (off by default).

#### Sleep Prevention During Downloads
To ensure large downloads finish uninterrupted, SAYL requests Windows to remain awake (`SetThreadExecutionState`) while downloads are actively running, and releases this lock automatically once all downloads stop or finish. This behavior is governed by the **"Prevent sleep during downloads"** setting and is enabled by default.

#### Shutdown Computer on Completion
An optional feature to shut down the machine automatically once all downloads complete, accessible from the floating download window and the toolbar's main menu (⋮):
- **Transient Preference:** Never saved to disk; automatically reset upon application restart to prevent unintended future shutdowns.
- **Safety Countdown:** Initiates a visible 60-second countdown prompt with an explicit **"Cancel"** action before initiating system shutdown.
- **Refusal Safeguards:** Strictly refuses to shut down if any task has failed, is paused, is still running, or if there are no download tasks in the list.

#### Save Folder Selection
- By default, files are saved to your system's `Downloads` folder.
- The destination directory for all downloads is configured in Settings using the native folder picker.

#### Pause and Resume
- **Pause:** Click the `Pause` button on any active download at any time. The engine gracefully stops workers and commits written bytes to disk.
- **Resume:** Click `Resume` to continue transferring from where you left off, provided the remote server supports range requests (`Accept-Ranges`) and the remote file validator has not changed.

---

### Browser Extension & Context Menu
SAYL integrates with Chromium-based browsers (Google Chrome, Microsoft Edge, Brave) via its companion extension:
- **Context Menu Integration:** Right-click any link on the web and select **"التحميل باستخدام سيل" / "Download with Sayl"** to send the URL directly to the SAYL desktop application.
- **Toolbar Popup:** Checks and displays whether the native messaging host (`com.sayl.downloader`) is connected, reporting the running app version.
- **Fallback Behavior:** If the SAYL application is closed or not installed, the popup indicates that the host is not connected and the browser leaves the link untouched, allowing the user to download through the browser as normal.

---

### Settings
Open Settings via the **gear button in the toolbar**:
- **Default Download Directory:** Configure where all downloaded files are saved using the native directory picker.
- **Concurrency & Connections:** Adjust maximum concurrent connections per download (default is 4).
- **Default View Mode:** Toggle between Card view and compact Table view, saved automatically.
- **Prevent Sleep During Downloads:** Keep Windows awake while downloads are active (enabled by default).
- **Show Download Window on Start:** Automatically display the floating window when a download begins (enabled by default).
- **Close Window on Finish:** Automatically close the floating download window when complete (disabled by default).
- **Appearance & Language:** Toggle between Arabic (with full RTL support) and English, and select Dark or Light theme modes.

---

### Frequently Asked Questions (FAQ)

#### Q: Does SAYL collect any data about my downloads or usage?
**A:** No. SAYL is built with a zero-telemetry policy. There are no tracking scripts, no third-party analytics, and no remote telemetry servers. All download records and URLs remain strictly on your local device.

#### Q: Why does a download occasionally restart from 0% instead of resuming?
**A:** This safeguards against file corruption if:
1. The remote web server does not support HTTP Range requests (`Accept-Ranges`).
2. The file on the remote server changed while paused (validator or `ETag` mismatch).

#### Q: Are downloaded parts lost if my computer crashes or loses power?
**A:** No. SAYL maintains a write-ahead journal and regularly flushes state to disk. When reopened, any incomplete downloads are restored and ready for resumption.

---

### System Requirements
- **Operating System:** Windows 10 (version 1903 or later) or Windows 11, 64-bit (x64).
- **Runtime:** Microsoft Edge WebView2 Runtime (pre-installed on modern Windows systems).
- **Storage:** Adequate free disk space and write permissions on the designated download directory.

---

### License & Website
- **License:** Licensed under the **Apache License, Version 2.0**. See [`LICENSE`](LICENSE) for details.
- **Official Website:** [https://sayl.site](https://sayl.site)
- **Repository & Issues:** [https://github.com/athar-studios/sayl](https://github.com/athar-studios/sayl)

