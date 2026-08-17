package com.be3.block.plugin;

import android.hardware.HardwareBuffer;
import android.os.ParcelFileDescriptor;

interface IPluginCallback {
    oneway void onPacket(in byte[] frame, in HardwareBuffer buffer, in ParcelFileDescriptor fence);
    oneway void onFailure(String message);
}
