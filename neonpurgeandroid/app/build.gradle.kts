plugins {
    id("com.android.application")
}

android {
    namespace = "com.lattenightstudio.neonpurge"
    compileSdk = 35

    defaultConfig {
        applicationId = "com.lattenightstudio.neonpurge"
        minSdk = 26
        targetSdk = 35
        versionCode = 1
        versionName = "1.4.0"

        ndk {
            // Phones, older phones and emulators. 32-bit x86 is not built by
            // modern Rust toolchains and is irrelevant for current devices.
            abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86_64")
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            isShrinkResources = false
            // Signing is done with apksigner after the build so the release
            // keystore never has to live in the repository.
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    packaging {
        jniLibs {
            useLegacyPackaging = false
        }
    }
}

// Compile the Rust cdylib for every shipped ABI and drop the .so files where
// Gradle expects them. `cargo ndk` finds the NDK through ANDROID_NDK_HOME.
val cargoNdkBuild by tasks.registering(Exec::class) {
    // Cargo.toml lives at the Gradle root, next to the `app` module.
    workingDir = rootProject.projectDir
    commandLine(
        "cargo", "ndk",
        // AAudio and libnativewindow only exist from API 26, so the linker
        // has to search the API 26 sysroot stubs, not the default ones.
        "--platform", "26",
        "-t", "arm64-v8a",
        "-t", "armeabi-v7a",
        "-t", "x86_64",
        "-o", "app/src/main/jniLibs",
        "build", "--release",
    )
}

tasks.named("preBuild") {
    dependsOn(cargoNdkBuild)
}
