Unicode true
RequestExecutionLevel user
Name "Snip2Pin"
!define APP_ICON "${__FILEDIR__}\..\..\assets\icons\line.ico"
Icon "${APP_ICON}"
UninstallIcon "${APP_ICON}"
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
Page components
Page directory
Page instfiles
UninstPage uninstConfirm
UninstPage instfiles

; Append mode requests write access without truncating or changing any bytes.
; Check existing payloads before replacing any file or publishing new metadata.
!macro CheckWritable FILE
    IfFileExists "$INSTDIR\${FILE}" 0 writable_${FILE}
    ClearErrors
    FileOpen $0 "$INSTDIR\${FILE}" a
    IfErrors install_failed
    FileClose $0
    writable_${FILE}:
!macroend

Section "Snip2Pin"
    SectionIn RO
    SetShellVarContext current
    SetRegView 64
    !insertmacro CheckWritable "snip2pin.exe"
    !insertmacro CheckWritable "LICENSE"
    !insertmacro CheckWritable "README.txt"
    !insertmacro CheckWritable "THIRD_PARTY_NOTICES.txt"
    !insertmacro CheckWritable "Uninstall.exe"
    ClearErrors
    SetOutPath "$INSTDIR"
    IfErrors install_failed
    ; Never ignore a failed overwrite or publish metadata for a failed extraction.
    SetOverwrite try
    File "${PAYLOAD}\snip2pin.exe"
    IfErrors install_failed
    File "${PAYLOAD}\LICENSE"
    IfErrors install_failed
    File "${PAYLOAD}\README.txt"
    IfErrors install_failed
    File "${PAYLOAD}\THIRD_PARTY_NOTICES.txt"
    IfErrors install_failed
    WriteUninstaller "$INSTDIR\Uninstall.exe"
    IfErrors install_failed
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
    IfErrors install_failed
    Goto install_complete
    install_failed:
        SetErrorLevel 1
        MessageBox MB_OK "Close Snip2Pin and check write access to the installation folder before installing." /SD IDOK
        Abort
    install_complete:
SectionEnd

Section "Desktop shortcut"
    SetShellVarContext current
    ClearErrors
    CreateShortcut "$DESKTOP\Snip2Pin.lnk" "$INSTDIR\snip2pin.exe"
    IfErrors 0 desktop_complete
        SetErrorLevel 1
        MessageBox MB_OK "Unable to create the desktop shortcut." /SD IDOK
        Abort
    desktop_complete:
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
    Delete "$DESKTOP\Snip2Pin.lnk"
    RMDir "$SMPROGRAMS\Snip2Pin"
    DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Snip2Pin"
    ; Deliberately non-recursive: preserve unrelated files and user settings.
    RMDir "$INSTDIR"
SectionEnd
