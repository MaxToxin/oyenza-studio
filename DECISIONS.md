# Решения по форку Oyenza Studio (островок Poni)

Репозиторий: `C:\Users\Toxin\oyenza-studio`, remote `MaxToxin/oyenza-studio`
(апстрим `lucacadalora/oyenza-studio`).

Рядом лежит `C:\Users\Toxin\poni-fork` — **старая копия** форка без эквалайзера.
Правки в неё не вносить, сборку из неё не ставить.

## Что добавлено в этом форке

### Эквалайзер из системного звука
- Полоски в островке берут звук с самого Windows (WASAPI loopback), а не из метаданных
  плеера, поэтому реагируют на любой звук: браузер, игра, плеер.
- Служебный поток `service equalizer`, около 44.1 кГц, 2 канала, F32.
- Полоски следуют за треком, а не за ползунком громкости Windows: loopback отдаёт звук
  уже после мастер-громкости, поэтому `viz.rs` умножает сигнал обратно на уровень
  устройства (мьют — тишина, усиление зажато до 20x, иначе в полоски лезет шумовой фон).
  Цена: два вызова COM на цикл опроса, не на кадр.
- Правки в `crates/poni/src/services/viz.rs` и `crates/poni/src/services/mic.rs`.
- Коммиты: `Equalizer from the PC's own sound`,
  `Equalizer behind the pill, scrim under the words, bars clear of the cover`,
  `Give the bars their dynamics back: tighter dB floor, slower reference`,
  `The bars follow the track, not the Windows volume slider`.

### Тексты Яндекс Музыки
- Источники текста: Яндекс, затем Apple Music, затем LRCLIB.
- Поиск трека анонимный, текст требует OAuth-токена и подписи запроса.
- Подпись — пара с десктопным клиентом: ключ `kzqU4XhfCaY6B6JTHODeq5` и заголовок
  `X-Yandex-Music-Client: YandexMusicDesktopAppWindows/5.95.0`.
- Записи Яндекса хранятся с `duration: None`: файл относится именно к этому `track_id`,
  иначе защита от расхождения в 6 секунд отбрасывает текст как «несинхронный».
- Токен лежит зашифрованным через DPAPI в `%LOCALAPPDATA%\Poni\yandex-music-token.bin`.
  Меню: `Yandex Music lyrics` → взять токен из буфера обмена / удалить токен.
- Пункт меню «открыть страницу, откуда берётся токен» — коммит
  `Yandex Music: a menu item that opens the page the token comes from`.

### Медиа-сессии (антимигание)
Правила в `crates/poni/src/services/media.rs`: приоритет не-браузерного плеера по AUMID,
отсутствие сброса кэша при пересоздании сессии тем же приложением, пауза 1.2 с перед сменой,
удержание кадра при пропаже источника, удержание кадра пока плеер пересоздаёт сессию.

## Сборка и установка

```bat
cd C:\Users\Toxin\oyenza-studio
build.bat                 :: cargo build --release -p poni
```

Затем закрыть островок, скопировать `target\release\poni.exe` в
`%LOCALAPPDATA%\Programs\Poni\poni.exe` и запустить. Предыдущая сборка сохраняется
рядом как `poni.exe.ponifork-old`, оригинал апстрима — `poni.exe.orig`.

Автозапуск: `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, значение `Poni`.
Оно указывает на путь, поэтому замена файла ничего не ломает.

## Обновления апстрима

Установщик апстрима (`Oyenza-Studio-Setup-*.exe`) затирает форк — обновляться только
через мерж `origin/main` в этот репозиторий и пересборку.

В 0.2.26 полезного для островка нет: там только заглушки Studio Pro, переименование
тарифа Claude и мелочи 0.2.25.

## Известное состояние

- В рабочем дереве есть незакоммиченные правки `mic.rs` и `viz.rs` — они попали в
  установленную сборку, но не в коммиты.
- Установленная сборка: 0.2.25 + этот форк, SHA256 начинается на `F7CA51F1`.
