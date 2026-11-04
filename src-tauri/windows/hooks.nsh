; Дополнительные шаги установщика NSIS (Tauri installerHooks).
; PRODUCTNAME и MAINBINARYNAME определены в шаблоне установщика Tauri.

!macro NSIS_HOOK_POSTINSTALL
  ; Ярлык на рабочем столе
  CreateShortCut "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe" "" "$INSTDIR\${MAINBINARYNAME}.exe" 0
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; Ярлык удаляем всегда. Настройки (%APPDATA%\com.lunexa.gtavlauncher) удаляются,
  ; только если в деинсталляторе отмечено «Удалить данные приложения».
  Delete "$DESKTOP\${PRODUCTNAME}.lnk"
!macroend
