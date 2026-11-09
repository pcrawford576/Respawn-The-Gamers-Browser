plugins {
    id("com.android.application")
}

android {
    namespace = "gg.respawn.browser"
    // Google Play requires new apps and updates to target Android 16 (API 36).
    compileSdk = 36

    defaultConfig {
        applicationId = "gg.respawn.browser"
        minSdk = 26
        targetSdk = 36
        versionCode = 2
        versionName = "0.1.1"
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
        }
    }
    // Kotlin is built into the Android Gradle Plugin 9; it targets the same Java version as below.
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

dependencies {
    implementation("androidx.core:core-ktx:1.15.0")
    implementation("androidx.appcompat:appcompat:1.7.0")
    implementation("com.google.android.material:material:1.12.0")
    implementation("androidx.drawerlayout:drawerlayout:1.2.0")
    // Per-tab login profiles, request header control.
    implementation("androidx.webkit:webkit:1.12.1")
}
