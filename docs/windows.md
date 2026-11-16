# volta для Windows

`volta.exe` — 64-битное приложение для Windows 10 и 11. Рядом с ним ничего
не нужно: он использует только системные библиотеки, а шрифт DejaVu с
кириллицей встроен в сам файл.

## Запуск

Двойной клик по `volta.exe` открывает окно. При первом запуске Windows может
показать предупреждение SmartScreen, потому что файл не подписан:
«Подробнее» → «Выполнить в любом случае».

Приложение запускается как консольная программа, поэтому из PowerShell или
`cmd` работают и обычные команды:

```powershell
.\volta.exe plan "Датчик газа MQ-2 включает вентилятор"
.\volta.exe render "Схема светодиода с кнопкой" --out Pictures\volta
.\volta.exe list
```

## Где лежат файлы

| Что | Где |
|---|---|
| Проекты и автосохранение | `%LOCALAPPDATA%\volta\projects` |
| Настройки (тема, провода, черновик) | `%APPDATA%\volta` |
| Экспорт по умолчанию | `%USERPROFILE%\Pictures\volta` |

Проект — обычный `*.volta.json`, его можно копировать и открывать на другой
машине. При выходе мастерская пишется в `autosave.volta.json`.

## Экспорт PNG

SVG и скетч сохраняются всегда. Для PNG нужен `rsvg-convert`, которого в
Windows нет по умолчанию; без него приложение честно скажет об этом и
предложит SVG.

Варианты:

- поставить librsvg через MSYS2:
  `pacman -S mingw-w64-ucrt-x86_64-librsvg`, затем добавить
  `C:\msys64\mingw64\bin` в `PATH`;
- или открыть сохранённый SVG в браузере и сохранить картинку оттуда.

## Сборка из исходников

На Windows с Visual Studio Build Tools (MSVC):

```powershell
.\bin\build-windows.ps1
```

Скрипт соберёт `target\windows\volta.exe` и упакует
`target\windows\volta-windows-x86_64.zip`.

Кросс-сборка из Linux с LLVM-MinGW (Linux-хост, Windows-цель):

```sh
bin/build-windows.sh
```

Скрипт ищет LLVM-MinGW в `~/.local/toolchains/llvm-mingw-*` или в
`$LLVM_MINGW`. Готовые сборки лежат на
<https://github.com/mstorsjo/llvm-mingw/releases> (файл `*-ucrt-ubuntu-*-x86_64.tar.xz`).

## Тема

volta следует за Omarchy, но на Windows его файлов нет, поэтому берётся
встроенная светлая тема. Схема, проверки и экспорт от этого не зависят.
