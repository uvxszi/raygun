package app.raygun.audio

import android.app.ApplicationExitInfo
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

    @Test
    fun aStartEndedByAnUpdateForceStopOrSwipeIsNoCrash() {
        val startedAt = 1_794_432_000_000L
        val before = startedAt - 60_000L
        // Android's own reasons for an end that's no crash.
        for (reason in listOf(
            ApplicationExitInfo.REASON_USER_REQUESTED,
            ApplicationExitInfo.REASON_PACKAGE_UPDATED,
            ApplicationExitInfo.REASON_PACKAGE_STATE_CHANGE,
            ApplicationExitInfo.REASON_EXIT_SELF,
            ApplicationExitInfo.REASON_USER_STOPPED,
        )) {
            assertTrue("reason $reason", CrashLog.endedAsItMay(reason, startedAt, before))
        }
        // Updated or reinstalled after that start began, whatever Android says (or doesn't).
        assertTrue(CrashLog.endedAsItMay(null, startedAt, startedAt + 5_000L))
        assertTrue(CrashLog.endedAsItMay(ApplicationExitInfo.REASON_SIGNALED, startedAt, startedAt + 5_000L))
        // Crashes, signals and the unknown stay crashes.
        for (reason in listOf(
            ApplicationExitInfo.REASON_CRASH,
            ApplicationExitInfo.REASON_CRASH_NATIVE,
            ApplicationExitInfo.REASON_ANR,
            ApplicationExitInfo.REASON_SIGNALED,
            ApplicationExitInfo.REASON_LOW_MEMORY,
        )) {
            assertFalse("reason $reason", CrashLog.endedAsItMay(reason, startedAt, before))
        }
        assertFalse(CrashLog.endedAsItMay(null, startedAt, before))
        assertFalse(CrashLog.endedAsItMay(null, 0L, startedAt))
    }

    @Test
    fun rustsMarkerGivesThePidAndStart() {
        val marker = """{"startedAt":1794432126550,"version":"1.8.1","crashes":0,"suspects":["dev.a"],"pid":4321}"""
        assertEquals(4321 to 1_794_432_126_550L, CrashLog.markerFields(marker))
        assertEquals(null to 0L, CrashLog.markerFields("{}"))
    }
}