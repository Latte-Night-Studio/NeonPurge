# NEON PURGE для Android

Нативний порт тієї самої гри на Android: та сама симуляція, ті самі дев'ять
видів зброї, хвилі, вороги, HUD, меню та локалізація. Відрізняється лише те, що
замість вікна `minifb` гру малює `ANativeWindow`, замість клавіатури й миші
працюють дотики, а звук іде через AAudio.

Гра повністю офлайнова: жодних дозволів, реклами, аналітики чи мережевих
запитів.

## Завантаження

Готовий APK лежить у розділі Releases:

| Файл | Що це |
| --- | --- |
| **NeonPurge-android-1.4.0.apk** | Підписаний release-APK для Android 8.0+ |

Тег: [`v1.4.0-android`](https://github.com/Latte-Night-Studio/NeonPurge/releases/tag/v1.4.0-android).

Встановлення:

1. Скопіюйте APK на пристрій або виконайте
   `adb install -r NeonPurge-android-1.4.0.apk`.
2. Якщо Android блокує встановлення з невідомого джерела, дозвольте його для
   файлового менеджера чи браузера, з якого відкрито APK.
3. Запустіть **Neon Purge** зі списку програм.

Мінімальна версія — **Android 8.0 (API 26)**, бо гра використовує AAudio.
Орієнтація — горизонтальна (працює в обидва боки), екран повноекранний,
системні панелі приховані.

## Керування

Меню працюють дотиком: торкніться кнопки, як звичайно.

| Дія | Жест або кнопка |
| --- | --- |
| Рух | Потягніть пальцем у лівій половині екрана |
| Приціл і вогонь | Потягніть пальцем у правій половині екрана — гра автоматично стріляє |
| Вибір зброї | Торкніться слота `1`–`9` у смузі вгорі |
| Перезарядка | Кнопка **RELOAD** |
| Ривок | Кнопка **DASH** |
| Наступна зброя | Кнопка **SWAP** |
| Пауза | Кнопка **PAUSE** у правому верхньому куті |
| Назад / вихід | Системна кнопка «Назад»: пауза, повернення з налаштувань, вихід із гри на головному екрані |

Віртуальні стіки з'являються там, де ви торкаєтеся екрана, тому великий палець
не потрібно шукати фіксовану зону. Лівий стік дає рух у восьми напрямках, правий
задає напрямок прицілу; поки палець на правій половині, зброя стріляє
безперервно (для напівавтоматичної зброї — з її власним темпом).

На відміну від десктопа, на Android немає вибору розміру вікна: гра завжди
повноекранна. Буфер 1280×720 вписується в екран із збереженням пропорцій, тож
на витягнутих екранах з'являються чорні смуги, а не розтягнуті пікселі.

## Налаштування та збереження

У меню налаштувань на Android залишилися лише **мова** (українська або
англійська) і **звук**. Режим вікна та роздільна здатність недоступні, бо їх
визначає система.

Налаштування та рекорд зберігаються у приватній папці застосунку
(`neon_purge_settings.txt` і `neon_purge_highscore.txt`). Вони зберігаються між
запусками, але видаляються разом із даними застосунку або після видалення гри.

## Збірка з джерел

### Що потрібно

- стабільний Rust із цілями `aarch64-linux-android`,
  `armv7-linux-androideabi`, `x86_64-linux-android`;
- Android SDK: platform 35, build-tools 35.0.0;
- Android NDK **27.2.12479018** (або новіший r27);
- JDK 17+ (перевірено на JDK 21);
- [`cargo-ndk`](https://github.com/bbqsrc/cargo-ndk) 4.x.

```powershell
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
cargo install cargo-ndk --locked
```

На Windows із GNU-тулчейном для host-залежностей збірки (`windows-sys`,
`getrandom`) потрібен `dlltool.exe` з MinGW у `PATH` — інакше компонувальник
скаржиться на `error calling dlltool`.

### APK

```powershell
$env:ANDROID_HOME = "$env:LOCALAPPDATA\Android\Sdk"
$env:ANDROID_NDK_HOME = "$env:ANDROID_HOME\ndk\27.2.12479018"

cd neonpurgeandroid
.\gradlew.bat assembleRelease
```

Gradle-задача `cargoNdkBuild` сама викликає `cargo ndk --platform 26` і кладе
`libneon_purge.so` для трьох ABI у `app/src/main/jniLibs`. Незабаром після
цього з'явиться `app/build/outputs/apk/release/app-release-unsigned.apk`.

`--platform 26` обов'язковий: `libaaudio.so` і `libnativewindow.so` існують
лише в sysroot-заглушках API 26+.

### Підпис

Ключ ніколи не лежить у репозиторії. Створіть його один раз поза проєктом:

```powershell
keytool -genkeypair -v -keystore neonpurge-release.jks `
  -alias neonpurge -keyalg RSA -keysize 4096 -validity 10000
```

Потім вирівняйте та підпишіть:

```powershell
$bt = "$env:ANDROID_HOME\build-tools\35.0.0"
$unsigned = "app\build\outputs\apk\release\app-release-unsigned.apk"
$signed = "release\NeonPurge-1.4.0-android.apk"

& "$bt\zipalign.exe" -c -P 16 4 $unsigned
& "$bt\apksigner.bat" sign --ks <шлях-до.jks> --ks-key-alias neonpurge `
  --min-sdk-version 26 --out $signed $unsigned
& "$bt\apksigner.bat" verify --verbose $signed
```

`zipalign -P 16` перевіряє 16-кілобайтне вирівнювання, потрібне для Android 15
на пристроях із 16 KB сторінками. APK підписується схемою v2/v3.

### Встановлення на пристрій

```powershell
adb install -r release\NeonPurge-1.4.0-android.apk
```

## Архітектура

```
neonpurgeandroid/
  Cargo.toml            cdylib `neon_purge`
  src/
    lib.rs              android_main, життєвий цикл, presenter
    touch.rs            віртуальні стіки, кнопки, смуга зброї
    storage.rs          шлях до приватної папки застосунку
  app/
    build.gradle.kts    SDK/NDK, ABI, задача cargo-ndk
    src/main/
      AndroidManifest.xml
      java/.../NeonPurgeActivity.java   immersive-режим
      res/              тема, іконка, назва
```

Ключове рішення — **не дублювати гру**. `lib.rs` підключає ті самі модулі, що й
десктоп, через `#[path = "../../src/..."]`:

- `game` — стан, симуляція, HUD і меню;
- `canvas`, `font`, `lang`, `math`, `ui`, `settings` — рендер, шрифт, локалізація,
  геометрія кнопок;
- `audio` — синтез звуку, з Android-гілкою AAudio.

Тому баланс, тексти, ефекти й вигляд збігаються з десктопною версією. Спільний
код отримав лише невеликі гачки: `crate::storage` для шляхів до файлів,
`Game::player_pos()` / `Game::weapon_index()` для дотикового шару та
`cfg!(target_os = "android")` для підказок і компактнішого меню налаштувань.

Рендер: гра малює кадр 1280×720 у той самий програмний буфер, що й на десктопі.
Presenter блокує буфер `ANativeWindow` у форматі `R8G8B8A8_UNORM`, вписує кадр
із збереженням пропорцій, заливає смуги чорним і конвертує `0xRRGGBB` у
RGBA-байти. Той самий масштаб використовує `touch.rs`, тому натискання завжди
збігаються з намальованими елементами.

Ввід: `android-activity` віддає `MotionEvent` із повним набором пальців. Кожен
палець отримує роль (лівий стік, правий стік, кнопка, слот зброї або меню) під
час натискання й утримує її до відпускання. Це дає коректний мультитач:
наприклад, рухатися лівим стіком, цілитися правим і тиснути **DASH** одночасно.

Звук: `src/audio.rs` синтезує ті самі семпли, що й на десктопі. Android-гілка
відкриває AAudio-потік (44.1 кГц, стерео, `PCM_Float`, low latency) і змішує
голоси у власному циклі; якщо потік відкрити не вдалося, гра працює без звуку,
а не падає.

## Перевірка

Без пристрою чи емулятора перевірено так:

- `cargo ndk --platform 26 -t arm64-v8a -t armeabi-v7a -t x86_64 build --release`
  — усі три ABI компілюються без попереджень;
- `./gradlew.bat assembleRelease` — Gradle-збірка проходить;
- `aapt2 dump badging` — пакет `com.lattenightstudio.neonpurge`, versionName
  `1.4.0`, minSdk 26, targetSdk 35, активність
  `com.lattenightstudio.neonpurge.NeonPurgeActivity`;
- в APK є `classes.dex` і `lib/{arm64-v8a,armeabi-v7a,x86_64}/libneon_purge.so`;
- `llvm-nm` бачить експортовані `ANativeActivity_onCreate` та `android_main`;
- `llvm-readelf` показує вирівнювання сегментів `0x4000` (16 KB);
- `apksigner verify` — підпис v2 і v3 валідний.

Спільний код перевірено на десктопі: `cargo test --release` (22 тести) і
`cargo run --release -- --smoke` проходять, тобто зміни для Android не змінили
поведінку Windows-версії.

Що варто перевірити на реальному пристрої після встановлення: чутливість
стіків, зручність розташування кнопок на конкретному екрані, затримку AAudio та
поведінку після блокування екрана.

## Версії

Версія Android-порту збігається з десктопною (`1.4.0`). `versionCode`
збільшується на кожен новий APK, інакше Android не дасть оновити застосунок
поверх встановленого.
