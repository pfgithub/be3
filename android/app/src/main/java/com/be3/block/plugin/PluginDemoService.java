package com.be3.block.plugin;

import android.app.Service;
import android.content.Intent;
import android.os.IBinder;

public final class PluginDemoService extends Service {
    private static final int MAX_PACKET_BYTES = 1024 * 1024 + 4;
    private volatile IPluginCallback callback;

    static { System.loadLibrary("plugin_demo"); }

    private final IPluginService.Stub binder = new IPluginService.Stub() {
        public void connect(IPluginCallback value) { callback = value; nativeStart(); }
        public void send(byte[] frame) {
            if (frame == null || frame.length > MAX_PACKET_BYTES) fail("Counter plugin packet was malformed");
            else nativeReceive(frame);
        }
        public void shutdown() { nativeShutdown(); stopSelf(); }
    };

    public IBinder onBind(Intent intent) { return binder; }
    public void onDestroy() { nativeShutdown(); super.onDestroy(); }

    private void fail(String message) { try { if (callback != null) callback.onFailure(message); } catch (Exception ignored) {} }
    private static native void nativeStart();
    private static native void nativeReceive(byte[] frame);
    private static native void nativeShutdown();
}
