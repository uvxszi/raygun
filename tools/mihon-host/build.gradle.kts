import java.security.MessageDigest

// Raygun's Mihon host: Mihon's source API (adapted, Apache-2.0), JVM stand-ins for the Android
// classes extensions touch, and a JSON-RPC loop over stdio. `mihon-host.jar` holds only these
// classes; the libraries below are fetched by Raygun on first use, each pinned by its SHA-256
// (`runtimeLibs` prints them for `raygun-app/src/mihon_container/tools.rs`).
plugins {
    kotlin("jvm") version "2.4.21"
    kotlin("plugin.serialization") version "2.4.21"
}

java {
    sourceCompatibility = JavaVersion.VERSION_17
    targetCompatibility = JavaVersion.VERSION_17
}

kotlin {
    compilerOptions {
        jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17)
        optIn.add("kotlin.concurrent.atomics.ExperimentalAtomicApi")
    }
}

dependencies {
    // The versions Mihon builds extensions against.
    implementation("com.squareup.okhttp3:okhttp:5.5.0")
    implementation("com.squareup.okhttp3:okhttp-brotli:5.5.0")
    implementation("org.jsoup:jsoup:1.23.2")
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.11.0")
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json-okio:1.11.0")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.11.0")
    implementation("io.reactivex:rxjava:1.3.8")
    implementation("uy.kohesive.injekt:injekt-core:1.16.1")
}

tasks.jar {
    archiveFileName.set("mihon-host.jar")
    manifest {
        attributes("Main-Class" to "raygun.mihon.host.MainKt")
    }
}

/** Each library the host runs with: `group:name:version file sha256`. */
tasks.register("runtimeLibs") {
    notCompatibleWithConfigurationCache("prints resolved artifacts")
    doLast {
        configurations.getByName("runtimeClasspath").resolvedConfiguration.resolvedArtifacts
            .sortedBy { it.file.name }
            .forEach { artifact ->
                val digest = MessageDigest.getInstance("SHA-256").digest(artifact.file.readBytes())
                val hex = digest.joinToString("") { "%02x".format(it) }
                println("${artifact.moduleVersion.id} ${artifact.file.name} $hex")
            }
    }
}
