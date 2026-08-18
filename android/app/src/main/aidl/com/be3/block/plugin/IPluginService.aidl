package com.be3.block.plugin;

import com.be3.block.plugin.IPluginCallback;

interface IPluginService {
    oneway void connect(IPluginCallback callback);
    oneway void send(in byte[] frame);
    oneway void shutdown();
}
