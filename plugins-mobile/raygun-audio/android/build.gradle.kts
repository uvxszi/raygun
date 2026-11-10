import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "app.raygun.audio"
    compileSdk = 37

    defaultConfig {
        minSdk = 24
        consumerProguardFiles("consumer-rules.pro")
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_1_8
        targetCompatibility = JavaVersion.VERSION_1_8
    }
}

kotlin {
    compilerOptions {
        jvmTarget = JvmTarget.JVM_1_8
    }
}

dependencies {
    implementation("androidx.core:core-ktx:1.16.0")
    implementation("androidx.appcompat:appcompat:1.7.1")
    implementation("androidx.media3:media3-exoplayer:1.11.1")
    implementation("androidx.media3:media3-session:1.11.1")
    // The video player (M38): stock controls, HLS and DASH streams.
    implementation("androidx.media3:media3-ui:1.11.1")
    implementation("androidx.media3:media3-exoplayer-hls:1.11.1")
    implementation("androidx.media3:media3-exoplayer-dash:1.11.1")
    // Video exports re-encoded to H.264 and AAC with the phone's own encoders (VideoConvert).
    implementation("androidx.media3:media3-transformer:1.11.1")
    implementation(project(":tauri-android"))
    // Tauri's own JSON mapper, for the commands Rust runs without the activity (DirectCalls);
    // tauri-android ships it at run time.
    compileOnly("com.fasterxml.jackson.core:jackson-databind:2.15.3")
}
