Unicode true
RequestExecutionLevel user
Name "Snip2Pin"
OutFile "${OUTPUT}"
InstallDir "$LOCALAPPDATA\Programs\Snip2Pin"
InstallDirRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Snip2Pin" "InstallLocation"
SetCompressor /SOLID lzma
VIProductVersion "${VERSION}.0"
VIAddVersionKey /LANG=1033 "ProductName" "Snip2Pin"
VIAddVersionKey /LANG=1033 "FileDescription" "Snip2Pin Setup"
VIAddVersionKey /LANG=1033 "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=1033 "LegalCopyright" "MIT License"

!ifdef SIGN_SCRIPT
; NSIS signs the complete uninstaller before embedding it, and checks exit status.
!uninstfinalize '"pwsh.exe" -NoProfile -NonInteractive -File "${SIGN_SCRIPT}" -Path "%1"' = 0
!endif

LicenseData "${PAYLOAD}\LICENSE"
Page license
Page directory
Page instfiles
UninstPage uninstConfirm
UninstPage instfiles

Section "Snip2Pin"
    SetShellVarContext current
    SetRegView 64
    SetOutPath "$INSTDIR"
    SetOverwrite on
    File "${PAYLOAD}\snip2pin.exe"
    File "${PAYLOAD}\LICENSE"
    File "${PAYLOAD}\README.txt"
    File "${PAYLOAD}\THIRD_PARTY_NOTICES.txt"
    WriteUninstaller "$INSTDIR\Uninstall.exe"
    CreateDirectory "$SMPROGRAMS\Snip2Pin"
    CreateShortcut "$SMPROGRAMS\Snip2Pin\Snip2Pin.lnk" "$INSTDIR\snip2pin.exe"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Snip2Pin" "DisplayName" "Snip2Pin"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Snip2Pin" "DisplayVersion" "${VERSION}"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Snip2Pin" "InstallLocation" "$INSTDIR"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Snip2Pin" "UninstallString" '$\"$INSTDIR\Uninstall.exe$\"'
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Snip2Pin" "QuietUninstallString" '$\"$INSTDIR\Uninstall.exe$\" /S'
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Snip2Pin" "DisplayIcon" "$INSTDIR\snip2pin.exe"
    WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Snip2Pin" "NoModify" 1
    WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Snip2Pin" "NoRepair" 1
SectionEnd

Section "Uninstall"
    SetShellVarContext current
    SetRegView 64
    ClearErrors
    Delete "$INSTDIR\snip2pin.exe"
    IfErrors 0 application_removed
        SetErrorLevel 1
        MessageBox MB_OK "Close Snip2Pin before uninstalling." /SD IDOK
        Abort
    application_removed:
    Delete "$INSTDIR\LICENSE"
    Delete "$INSTDIR\README.txt"
    Delete "$INSTDIR\THIRD_PARTY_NOTICES.txt"
    Delete "$INSTDIR\Uninstall.exe"
    Delete "$SMPROGRAMS\Snip2Pin\Snip2Pin.lnk"
    RMDir "$SMPROGRAMS\Snip2Pin"
    DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Snip2Pin"
    ; Deliberately non-recursive: preserve unrelated files and user settings.
    RMDir "$INSTDIR"
SectionEnd
