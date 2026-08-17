package com.be3.block.plugin;

import android.content.ComponentName;
import android.content.Context;
import android.content.Intent;
import android.content.ServiceConnection;
import android.hardware.HardwareBuffer;
import android.os.IBinder;
import android.os.ParcelFileDescriptor;

public final class PluginHostBridge {
    private static final int MAX_PACKET_BYTES = 1024 * 1024 + 4;
    private static IPluginService service;
    private static ServiceConnection connection;

    static { System.loadLibrary("block_app_lib"); }

    public static synchronized boolean bind(Context context) {
        if (connection != null || android.os.Build.VERSION.SDK_INT < 26) return false;
        connection = new ServiceConnection() {
            public void onServiceConnected(ComponentName name, IBinder binder) {
                service = IPluginService.Stub.asInterface(binder);
                try {
                    binder.linkToDeath(() -> disconnected("Plugin service process died"), 0);
                    service.connect(callback);
                    nativeConnected();
                } catch (Exception error) { disconnected(error.toString()); }
            }
            public void onServiceDisconnected(ComponentName name) { disconnected("Plugin service disconnected"); }
            public void onBindingDied(ComponentName name) { disconnected("Plugin service binding died"); }
            public void onNullBinding(ComponentName name) { disconnected("Plugin service returned no Binder"); }
        };
        Intent intent = new Intent(context, PluginDemoService.class);
        if (!context.bindService(intent, connection, Context.BIND_AUTO_CREATE)) {
            connection = null;
            return false;
        }
        return true;
    }

    public static synchronized void unbind(Context context) {
        if (service != null) try { service.shutdown(); } catch (Exception ignored) {}
        if (connection != null) context.unbindService(connection);
        service = null;
        connection = null;
    }

    private static final IPluginCallback callback = new IPluginCallback.Stub() {
        public void onPacket(byte[] frame, HardwareBuffer buffer, ParcelFileDescriptor fence) {
            try {
                if (frame == null || frame.length > MAX_PACKET_BYTES) nativeDisconnected("Plugin packet exceeded the size limit");
                else nativePacket(frame);
            } finally {
                if (buffer != null) buffer.close();
                if (fence != null) try { fence.close(); } catch (Exception ignored) {}
            }
        }
        public void onFailure(String message) { nativeDisconnected(message); }
    };

    private static synchronized void disconnected(String reason) {
        service = null;
        nativeDisconnected(reason);
    }

    private static native void nativeConnected();
    private static native void nativeDisconnected(String reason);
    private static native void nativePacket(byte[] packet);
}
