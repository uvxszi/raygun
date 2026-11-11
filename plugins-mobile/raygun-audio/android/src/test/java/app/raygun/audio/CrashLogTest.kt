package app.raygun.audio

import java.io.ByteArrayOutputStream
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class CrashLogTest {
    @Test
    fun recordsShowTheirTimeAsADateNotMilliseconds() {
        val text = "Raygun crash record\nKind: java\nTime: 1794432126550\nVersion: 1.8.0\n\nTime: 12 in the body stays"
        val shown = CrashLog.readable(text)
        assertFalse(shown.contains("1794432126550"))
        assertTrue(shown.lines()[2].startsWith("Time: "))
        assertTrue(shown.contains("2026"))
        assertTrue(shown.endsWith("Time: 12 in the body stays"))
    }

    @Test
    fun theAbortMessageIsReadFromATombstone() {
        // field 2 (build fingerprint), field 5 (pid, a varint), field 14 (abort message)
        val bytes = ByteArrayOutputStream().apply {
            write(byteArrayOf((2 shl 3 or 2).toByte(), 3))
            write("abc".toByteArray())
            write(byteArrayOf((5 shl 3).toByte(), 0x96.toByte(), 0x01))
            val message = "thread has overflowed its stack".toByteArray()
            write(byteArrayOf((14 shl 3 or 2).toByte(), message.size.toByte()))
            write(message)
        }.toByteArray()
        assertEquals("thread has overflowed its stack", Tombstone.abortMessage(bytes))
        assertNull(Tombstone.abortMessage(byteArrayOf((2 shl 3 or 2).toByte(), 90)))
    }
}
