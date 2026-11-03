; Only application-owned data is removed, never paths read from the library index.
!macro OpenPhotoDeleteDataFile NAME
  ${If} ${FileExists} "$LOCALAPPDATA\Open-Photo\${NAME}"
    ClearErrors
    Delete "$LOCALAPPDATA\Open-Photo\${NAME}"
    ${If} ${Errors}
      StrCpy $R9 1
    ${EndIf}
  ${EndIf}
!macroend
!macro OpenPhotoDeleteDatabase NAME
  !insertmacro OpenPhotoDeleteDataFile "${NAME}"
  !insertmacro OpenPhotoDeleteDataFile "${NAME}-wal"
  !insertmacro OpenPhotoDeleteDataFile "${NAME}-shm"
  !insertmacro OpenPhotoDeleteDataFile "${NAME}-journal"
!macroend
!macro NSIS_HOOK_PREUNINSTALL
  ; Explicit CLI consent permits isolated installer lifecycle acceptance in CI.
  ; Updates always retain user data, even if this option was passed accidentally.
  ${If} $UpdateMode <> 1
    Push $R0
    ClearErrors
    ${GetOptions} $CMDLINE "/PURGEAPPDATA=" $R0
    ${IfNot} ${Errors}
    ${AndIf} $R0 == "1"
      StrCpy $DeleteAppDataCheckboxState 1
    ${EndIf}
    Pop $R0
    ClearErrors
  ${EndIf}
!macroend
!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $DeleteAppDataCheckboxState = 1
  ${AndIf} $UpdateMode <> 1
    SetShellVarContext current
    Push $R9
    StrCpy $R9 0
    !insertmacro OpenPhotoDeleteDatabase "user-data.sqlite"
    !insertmacro OpenPhotoDeleteDatabase "media-index.sqlite"
    !insertmacro OpenPhotoDeleteDatabase "video-projects.sqlite"
    !insertmacro OpenPhotoDeleteDatabase "ai-embeddings.sqlite"
    !insertmacro OpenPhotoDeleteDatabase "ai-faces.sqlite"
    !insertmacro OpenPhotoDeleteDatabase "ai-ocr.sqlite"
    ${If} ${FileExists} "$LOCALAPPDATA\Open-Photo\ai-models\*.*"
      ClearErrors
      RmDir /r "$LOCALAPPDATA\Open-Photo\ai-models"
      ${If} ${Errors}
        StrCpy $R9 1
      ${EndIf}
    ${EndIf}
    ${If} ${FileExists} "$LOCALAPPDATA\Open-Photo\previews-v1\*.*"
      ClearErrors
      RmDir /r "$LOCALAPPDATA\Open-Photo\previews-v1"
      ${If} ${Errors}
        StrCpy $R9 1
      ${EndIf}
    ${EndIf}
    ; Never recursively remove this parent: it may also be the install directory
    ; and may contain other files that do not belong to Open-Photo.
    RmDir "$LOCALAPPDATA\Open-Photo"
    ClearErrors
    ${If} $R9 = 1
      IfSilent +2
        MessageBox MB_OK|MB_ICONEXCLAMATION "Some local Open-Photo data could not be removed. Close any other Open-Photo windows and check $LOCALAPPDATA\Open-Photo. Original media was not removed."
      SetErrorLevel 2
    ${EndIf}
    Pop $R9
  ${EndIf}
!macroend
