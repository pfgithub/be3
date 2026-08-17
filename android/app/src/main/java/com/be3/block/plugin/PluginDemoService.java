package com.be3.block.plugin;

import android.app.Service;
import android.content.Intent;
import android.hardware.HardwareBuffer;
import android.os.IBinder;
import android.os.ParcelFileDescriptor;
import java.util.concurrent.ArrayBlockingQueue;
import java.util.concurrent.BlockingQueue;

public final class PluginDemoService extends Service {
    private static final int MAX_PACKET_BYTES = 1024 * 1024 + 4;
    private static final int MAX_PACKETS = 256;
    private final BlockingQueue<Packet> packets = new ArrayBlockingQueue<>(MAX_PACKETS);
    private volatile IPluginCallback callback;

    static { System.loadLibrary("plugin_demo"); }

    private final IPluginService.Stub binder = new IPluginService.Stub() {
        public void connect(IPluginCallback value) { callback = value; nativeStart(); }
        public void send(byte[] frame, HardwareBuffer buffer, ParcelFileDescriptor fence) {
            if (frame == null || frame.length > MAX_PACKET_BYTES || !packets.offer(new Packet(frame, buffer, fence))) {
                close(buffer, fence);
                fail("Plugin packet was malformed or the queue is full");
            }
        }
        public void shutdown() { nativeShutdown(); stopSelf(); }
    };

    public IBinder onBind(Intent intent) { return binder; }
    public void onDestroy() { nativeShutdown(); super.onDestroy(); }

    private void fail(String message) { try { if (callback != null) callback.onFailure(message); } catch (Exception ignored) {} }
    private static void close(HardwareBuffer buffer, ParcelFileDescriptor fence) {
        if (buffer != null) buffer.close();
        if (fence != null) try { fence.close(); } catch (Exception ignored) {}
    }

    private static final class Packet {
        final byte[] frame;
        final HardwareBuffer buffer;
        final ParcelFileDescriptor fence;
        Packet(byte[] frame, HardwareBuffer buffer, ParcelFileDescriptor fence) { this.frame = frame; this.buffer = buffer; this.fence = fence; }
    }

    private static native void nativeStart();
    private static native void nativeShutdown();
}
