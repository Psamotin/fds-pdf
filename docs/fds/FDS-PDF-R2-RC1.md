# FDS-PDF-R2-RC1

Продукт: **ФДС ПДФ**, версия **0.2.0-fds.1-rc1**. Это кандидат для ручной
приёмки Windows 11 x64; main не следует сливать до приёмки.

## Объём изменений

Ветка `fds-pdf-r2-ocr-rc1` продолжает R1: русская локализация со стабильными
ключами в `resources.rs`, русским языком по умолчанию и английским fallback;
внутренние command IDs, имена crates и Cargo binary остаются прежними.
В пользовательском UI отсутствуют upstream marketing/community CTA.
Добавлено 24 ключа для локального OCR. Форматируемые строки проверяет
`cargo run -p printcraft-ui-egui --example l10n -- --check`.

Утверждённый `FDS-PDF.png` взят из предоставленного владельцем архива
`FDS-PDF.png.zip`. Исходные байты сохранены в `assets/fds-pdf/FDS-PDF.png`:
SHA-256 `06a539c67d15487a52d9712f78f137d9bb5016fc5a4205b53b29809083510bed`.
`cargo xtask fds-icon` воспроизводимо создаёт PNG окна и ICO с размерами
16/24/32/48/64/128/256. Изображение вписано с сохранением пропорций.
Иконка встроена в Windows exe и окно; в комплекте есть ICO для ярлыков.
ArtCraft logo не используется. Лицензии и attribution сохранены.

Cargo binary называется `printcraft`, а workflow копирует его как
`FDS-PDF.exe`. Так сохраняются upstream build targets и внутренние имена,
при этом корпоративный комплект имеет нужное имя. Windows ProductName и
ProductVersion проверяются после сборки.

## Локальный OCR

`crates/ui-egui/src/local_ocr.rs` — внешний desktop-адаптер. Он получает снимок
байтов документа, создаёт приватный временный input, вызывает процесс и
атомарно публикует новый PDF через `persist_noclobber`. Исходный файл, история
редактирования и обычные методы открытия/сохранения не меняются.
`local_ocr_ui.rs` подключает адаптер к существующему Scan & OCR диалогу.
Windows приложение включает его при старте; исходный Rust OCR остаётся
нетронутым для upstream/web и остальных платформ.

Все пути runtime вычисляются от `current_exe`, независимо от рабочего каталога:

```text
FDS-PDF.exe
OCR/Python/python.exe (+ stdlib, DLLs, Lib/site-packages)
OCR/Tesseract-OCR/tesseract.exe (+ проверенные DLLs)
OCR/Tesseract-OCR/tessdata/{rus,eng,osd}.traineddata
LICENSES/ (лицензии, уведомления и SOURCES)
THIRD-PARTY-NOTICES.txt
README.txt
SHA256SUMS.json
```

Профиль по умолчанию (аргументы передаются напрямую, без shell):

```text
python.exe -I -m ocrmypdf --language rus+eng --mode skip --rotate-pages --deskew
  --rasterizer pypdfium --output-type pdf --optimize 0 input.pdf output.pdf
```

`-I` и относительный `python313._pth` изолируют Python. PATH и TESSDATA_PREFIX
задаются только дочернему процессу; глобальное окружение не меняется.
CREATE_NO_WINDOW скрывает непосредственные дочерние консоли. Дополнительный
`sitecustomize.py` в закреплённом Python runtime включает этот флаг для
_CreateProcess дочерних процессов Python, включая Tesseract и multiprocessing;
сам флаг не наследуется автоматически. Политика активна только при
FDS_OCR_HIDE_CHILDREN=1 в окружении OCR. Исходники OCRmyPDF не изменяются.
Windows smoke проверяет загрузку политики и отсутствие консоли у вложенного
процесса с исходными creationflags=0. Безопасная библиотека `win32job`
объединяет Windows процессы в Job Object с kill-on-close, чтобы отмена
завершала весь OCR процесс вместе с дочерними workers.

Проверка runtime выполняется в фоне: непустые бинарники/модели, Python import
и точные версии Python/OCRmyPDF, Tesseract version и наличие rus/eng/osd.
GUI показывает статус проверки, настройки поворота/выравнивания, ход
операции, число файлов, время и отмену. Ошибки русские, диагностика свёрнута.
Результат — `<имя>_OCR.pdf`, затем `_OCR_2.pdf` и т. д.; после успеха доступна
кнопка «Открыть результат». Документ без пути получает диалог выбора файла.

Обработка локальная: нет сетевых запросов, загрузок runtime, телеметрии или
логирования содержимого документов. stdout/stderr ограничены 64 KiB каждый
и используются только для диагностики; содержимое документов не пишется в лог.

## Runtime и распространение

`packaging/fds/runtime-lock.json` закрепляет официальные URLs, версии и SHA-256.
`requirements-windows.lock` содержит 28 exact-version wheels; установка на
runner выполняется offline с `--require-hashes --no-deps --no-index`.
На рабочем ПК установка и доступ к Интернету не требуются.

- Python 3.13.15 x64: официальный installer в архиве actions/python-versions.
  Устанавливается только на CI в каталог комплекта, без PATH/launcher/ярлыков.
- OCRmyPDF 17.4.0: PyPI wheel и закреплённая dependency closure.
- Tesseract 5.5.3.20260724: официальный release tesseract-ocr/tesseract,
  SHA-256 `bee9e3434bd94fd65387d9be28cd467a41f61b1275383b55b0f59a1331270ae4`.
  Installer не запускается: 7-Zip извлекает проверенный runtime dependency
  closure без installer plugins, training tools и лишних библиотек GTK/Pango.
- Модели rus/eng/osd: tessdata_fast revision
  `87416418657359cb625c412a48b6e1d6d41c29bd`.

Tesseract содержит GPL/LGPL DLL dependencies; это не пакет, целиком покрытый
Apache-2.0. Комплект сохраняет отдельные лицензионные условия, соответствующие
исходники Python packages, native libraries и рецепты сборки с patches.
JBIG-KIT допускает GPL-2.0-or-later; для внешнего связанного runtime используется
вариант GPL-3.0. LGPL права на модификацию/замену библиотек не ограничиваются.
GCC Runtime Library Exception также сохранено. Исходный Rust PDF engine
не получает copyleft код. Подробнее: `packaging/fds/THIRD-PARTY-NOTICES.txt`.

## Проверки

Локальные проверки Rust 1.95:

- `cargo fmt --all -- --check`;
- `cargo check --workspace --locked`;
- `cargo test --workspace --locked`: 713 PASS, 0 FAIL, 4 ignored;
- `cargo clippy --locked -p printcraft-ui-egui -p printcraft --all-targets --no-deps -- -D warnings`;
- `cargo build --release --locked -p printcraft`;
- `cargo check --locked -p printcraft-ui-egui --target wasm32-unknown-unknown`;
- генератор l10n, `cargo xtask assets`, `cargo xtask fds-icon --check`, actionlint;
- Windows adapter отдельно проверен компиляцией для x86_64-pc-windows-msvc;
- реальные egui UI tests: русский диалог и отсутствие runtime без изменения документа;
- процессные unit tests без OCR: командные аргументы, окружение, ошибки,
  отмена, коллизии и атомарная публикация.

Full workspace Clippy имеет исходные upstream `collapsible_match` замечания,
в том числе `crates/a11y/src/alt.rs`, `crates/optimize/src/images.rs` и
`xtask/src/layers.rs`; они не исправлялись в этой задаче. Проверка wasm имеет
исходные warnings; Windows release не подтверждается Linux проверками.

Windows workflow `.github/workflows/fds-windows-build.yml` дополнительно:

1. Проверяет версию, fmt и desktop tests; выполняет release build --locked.
2. Упаковывает точный runtime с лицензиями и исходниками.
3. Переносит комплект в путь с кириллицей/пробелами, изолирует host PATH.
4. Проверяет версии, модели, import и searchable PDF на синтетическом скане;
   SHA-256 исходного PDF должен остаться прежним.
5. Проверяет ProductVersion exe и Tesseract; только затем загружает
   `FDS-PDF-0.2.0-fds.1-rc1-windows-x64` (ZIP и build-report.json с размером/hash).
6. При push этой ветки создаёт/обновляет draft PR в main после успешной сборки,
   без merge. Для этого необходима настройка GitHub, разрешающая Actions
   создавать PR; запрет этой настройки не отменяет уже собранный artifact.

## Ограничения и приёмка

Этот документ описывает gates workflow; фактический Windows PASS подтверждается
конкретным run и artifact, а не наличием YAML. Эта Linux среда не запускает
Windows runtime. Некоторые официальные source sites и GitHub API закрыты
сетевым шлюзом; закреплённые hashes не обходят такое ограничение.

OCR моделей старого Rust backend нет в этой среде: его model-dependent tests
имеют существующий ранний выход. Четыре явно ignored tests также не выполнялись.
При ошибке в середине пакетного OCR уже завершённые новые файлы остаются на
диске; итоговый диалог сообщает ошибку. Главный engine и исходники не меняются.

Ручная приёмка Windows 11 на ПК без установленного OCR:

- Распаковать полный ZIP в папку с кириллицей и пробелами, отключить Интернет.
- Запустить FDS-PDF.exe; проверить имя, иконку и отсутствие консоли.
- Проверить русский Home, Все инструменты, вкладки, меню и диалоги.
- Открыть обычный PDF, сохранить копию; проверить отсутствие регрессий.
- Распознать скан с русским и английским текстом; открыть результат и проверить
  поиск/копирование текста, страницы и целостность исходного файла.
- Повторить OCR при существующем *_OCR.pdf: прежний результат не перезаписывается.
- Проверить отмену на большом документе и отсутствие оставшихся OCR processes.
- Проверить ошибку для защищённого/повреждённого PDF и временно отсутствующей
  модели; восстановить полный runtime после проверки.
- Проверить запуск на Intel GPU и сохранение DX12/OpenGL fallback без Vulkan.

NEXT_STEP: **MANUAL_WINDOWS_ACCEPTANCE_FDS_PDF_R2_RC1**.
