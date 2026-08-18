package com.be3.block.plugin;

interface IPluginCallback {
    oneway void onPacket(in byte[] frame);
    oneway void onFailure(String message);
}
