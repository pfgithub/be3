package com.be3.block.plugin;

import android.content.ComponentName;
import android.content.Context;
import android.content.Intent;
import android.content.ServiceConnection;
import android.os.IBinder;

import java.util.HashMap;
import java.util.Map;

public final class PluginHostBridge {
    private static final int MAX_PACKET_BYTES = 1024 * 1024 + 4;
    private static final Map<String, Runtime> runtimes = new HashMap<>();

    static { System.loadLibrary("block_app_lib"); }

    private static final class Runtime {
        IPluginService service;
        ServiceConnection connection;
    }

    public static synchronized boolean bind(Context context, String plugin, String serviceClass) {
        if (runtimes.containsKey(plugin) || android.os.Build.VERSION.SDK_INT < 26) return false;
        Intent intent;
        try {
            intent = new Intent(context, Class.forName(serviceClass));
        } catch (ClassNotFoundException error) {
            return false;
        }
        Runtime runtime = new Runtime();
        runtime.connection = new ServiceConnection() {
            public void onServiceConnected(ComponentName name, IBinder binder) {
                try {
                    binder.linkToDeath(() -> disconnected(plugin, "The plugin process died"), 0);
                    IPluginService service = IPluginService.Stub.asInterface(binder);
                    service.connect(callback(plugin));
                    connected(plugin, service);
                } catch (Exception error) { disconnected(plugin, error.toString()); }
            }
            public void onServiceDisconnected(ComponentName name) { disconnected(plugin, "The plugin disconnected"); }
            public void onBindingDied(ComponentName name) { disconnected(plugin, "The plugin binding died"); }
            public void onNullBinding(ComponentName name) { disconnected(plugin, "The plugin returned no Binder"); }
        };
        runtimes.put(plugin, runtime);
        if (!context.bindService(intent, runtime.connection, Context.BIND_AUTO_CREATE)) {
            runtimes.remove(plugin);
            return false;
        }
        return true;
    }

    public static synchronized void send(String plugin, byte[] frame) throws Exception {
        Runtime runtime = runtimes.get(plugin);
        if (runtime == null || runtime.service == null) throw new IllegalStateException("The plugin is not connected");
        runtime.service.send(frame);
    }

    public static synchronized void unbind(Context context, String plugin) {
        Runtime runtime = runtimes.remove(plugin);
        if (runtime == null) return;
        if (runtime.service != null) try { runtime.service.shutdown(); } catch (Exception ignored) {}
        if (runtime.connection != null) context.unbindService(runtime.connection);
    }

    private static IPluginCallback callback(String plugin) {
        return new IPluginCallback.Stub() {
            public void onPacket(byte[] frame) {
                if (frame == null || frame.length > MAX_PACKET_BYTES) nativeDisconnected(plugin, "A plugin packet exceeded the size limit");
                else nativePacket(plugin, frame);
            }
            public void onFailure(String message) { nativeDisconnected(plugin, message); }
        };
    }

    private static synchronized void connected(String plugin, IPluginService service) {
        Runtime runtime = runtimes.get(plugin);
        if (runtime == null) return;
        runtime.service = service;
        nativeConnected(plugin);
    }

    private static synchronized void disconnected(String plugin, String reason) {
        Runtime runtime = runtimes.get(plugin);
        if (runtime != null) runtime.service = null;
        nativeDisconnected(plugin, reason);
    }

    private static native void nativeConnected(String plugin);
    private static native void nativeDisconnected(String plugin, String reason);
    private static native void nativePacket(String plugin, byte[] packet);
}
