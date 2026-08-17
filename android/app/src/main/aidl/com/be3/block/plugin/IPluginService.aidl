package com.be3.block.plugin;

import android.hardware.HardwareBuffer;
import android.os.ParcelFileDescriptor;
import com.be3.block.plugin.IPluginCallback;

interface IPluginService {
    oneway void connect(IPluginCallback callback);
    oneway void send(in byte[] frame, in HardwareBuffer buffer, in ParcelFileDescriptor fence);
    oneway void shutdown();
}
