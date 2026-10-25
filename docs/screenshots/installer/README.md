# Installer dialog renders

Layout-accurate mock renders composed from the real installer artwork
(apps/desktop/src-tauri/icons/*.bmp) and the real MSI dialog control geometry
(Control and TextStyle tables of the built v0.1.0 MSI), plus the NSIS MUI2
welcome/sidebar/header geometry from the bundled NSIS 3.x sources.

- msi-welcome.png    - MSI WelcomeDlg (bitmap dialog, wix-dialog.bmp background)
- msi-license.png    - MSI LicenseAgreementDlg (banner dialog, wix-banner.bmp)
- msi-install-folder.png - MSI InstallDirDlg (banner dialog, wix-banner.bmp)
- msi-finish.png     - MSI ExitDialog with the Launch SAYL checkbox
- nsis-welcome.png   - NSIS MUI2 welcome page (nsis-sidebar.bmp)
- nsis-directory.png - NSIS MUI2 directory page (nsis-header.bmp)

These are NOT runtime captures. Runtime capture is blocked on this developer
machine per the project workflow (validation is CI-only; see docs/TOOLCHAIN.md
and STATUS.json): the real wizard appearance is verified by building the
installers in CI and by the pixel/contrast gates in
tools/check-installer-artwork.ps1.
