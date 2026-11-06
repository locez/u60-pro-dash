# 旧网关入口兼容目录

维护入口已改为 [Tailscale 子网网关](../tailscale-gateway/README.md)。这里的脚本只转到新实现，供已有命令和旧版 agent 调用。

新部署需要显式指定 `--remote-cidr`；不再假定某个使用地点或固定远端网段。设备已有的旧规则脚本和备份不会因 Git 更新而改变。旧的 `/data/u60-home-gateway-backup` 会被新工具识别并用于回滚。
