import { useEffect, useState } from "react";
import {
  ArrowRight,
  Check,
  Globe2,
  Info,
  LoaderCircle,
  Network,
  Plus,
  RefreshCw,
  Route,
  ShieldCheck,
  X,
} from "lucide-react";
import { api } from "./api";
import { Dialog } from "./Dialog";
import { IconButton } from "./IconButton";
import { errorText } from "./types";

type Settings = { proxy: string; bypass: string };
const domains = (value: string) => [
  ...new Set(
    value
      .split(/[,，\s]+/)
      .filter(Boolean)
      .map((host) => host.toLowerCase()),
  ),
];

export function NetworkSettingsDialog({ onClose }: { onClose: () => void }) {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [saved, setSaved] = useState<Settings | null>(null);
  const [manual, setManual] = useState(false);
  const [draft, setDraft] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [retry, setRetry] = useState(0);
  useEffect(() => {
    let cancelled = false;
    setError("");
    void api
      .networkSettings()
      .then((value) => {
        if (!cancelled) {
          setSettings(value);
          setSaved(value);
          setManual(Boolean(value.proxy));
        }
      })
      .catch((e) => {
        if (!cancelled) setError(errorText(e));
      });
    return () => {
      cancelled = true;
    };
  }, [retry]);
  const hosts = domains(settings?.bypass ?? "");
  const next: Settings = {
    proxy: manual ? (settings?.proxy.trim() ?? "") : "",
    bypass: domains([...hosts, draft].join(",")).join(","),
  };
  const dirty =
    saved && (next.proxy !== saved.proxy || next.bypass !== saved.bypass);
  const addDomains = (value: string) => {
    if (!settings) return;
    setSettings({
      ...settings,
      bypass: domains([...hosts, value].join(",")).join(","),
    });
    setDraft("");
    setError("");
  };
  return (
    <Dialog label="网络设置" dismissible={!busy} onClose={onClose}>
      <form
        className="modal network-settings"
        onSubmit={async (e) => {
          e.preventDefault();
          if (!settings || busy) return;
          setBusy(true);
          setError("");
          try {
            await api.saveNetworkSettings(next);
            onClose();
          } catch (e) {
            setError(errorText(e));
          } finally {
            setBusy(false);
          }
        }}
      >
        <header className="network-settings-heading">
          <span className="network-settings-mark">
            <Network size={23} aria-hidden="true" />
          </span>
          <div>
            <h2>网络设置</h2>
            <p>为不同的仓库，选择合适的连接方式。</p>
          </div>
          <IconButton
            label="关闭网络设置"
            icon={X}
            disabled={busy}
            onClick={onClose}
          />
        </header>
        <div className="network-settings-body">
          {settings ? (
            <>
              <div className="network-mode" role="group" aria-label="连接方式">
                <button
                  type="button"
                  aria-pressed={!manual}
                  disabled={busy}
                  onClick={() => {
                    setManual(false);
                    setError("");
                  }}
                >
                  <Globe2 size={17} aria-hidden="true" />
                  <span>
                    <strong>沿用 Git 配置</strong>
                    <small>使用已有的网络配置</small>
                  </span>
                  {!manual && <Check size={15} aria-hidden="true" />}
                </button>
                <button
                  type="button"
                  aria-pressed={manual}
                  disabled={busy}
                  onClick={() => {
                    setManual(true);
                    setError("");
                  }}
                >
                  <Route size={17} aria-hidden="true" />
                  <span>
                    <strong>手动代理</strong>
                    <small>代理与直连分别设置</small>
                  </span>
                  {manual && <Check size={15} aria-hidden="true" />}
                </button>
              </div>
              <section
                className={"network-card" + (!manual ? " is-inactive" : "")}
                aria-labelledby="proxy-section-title"
              >
                <div className="network-card-heading">
                  <span className="network-card-icon">
                    <Globe2 size={17} aria-hidden="true" />
                  </span>
                  <div>
                    <h3 id="proxy-section-title">代理连接</h3>
                    <p>用于需要代理访问的 HTTP / HTTPS 仓库</p>
                  </div>
                  <span className="network-card-tag">
                    {manual ? "手动配置" : "沿用配置"}
                  </span>
                </div>
                <label className="network-field-label" htmlFor="network-proxy">
                  代理地址
                </label>
                <div className="network-proxy-input">
                  <Route size={16} aria-hidden="true" />
                  <input
                    id="network-proxy"
                    disabled={busy || !manual}
                    required={manual}
                    value={settings.proxy}
                    placeholder="http://127.0.0.1:12345"
                    spellCheck={false}
                    aria-describedby="network-proxy-help"
                    onChange={(e) => {
                      setSettings({ ...settings, proxy: e.target.value });
                      setError("");
                    }}
                  />
                </div>
                <p className="network-field-help" id="network-proxy-help">
                  {manual
                    ? "支持 HTTP、HTTPS、SOCKS5 / SOCKS5H，不填账号密码。"
                    : "沿用 Git 配置与应用启动时的代理环境；桌面应用通常不会继承终端代理。"}
                </p>
              </section>
              <section
                className={"network-card" + (!manual ? " is-inactive" : "")}
                aria-labelledby="bypass-section-title"
              >
                <div className="network-card-heading">
                  <span className="network-card-icon direct">
                    <ArrowRight size={17} aria-hidden="true" />
                  </span>
                  <div>
                    <h3 id="bypass-section-title">直连规则</h3>
                    <p>这些域名绕过上方代理，直接连接</p>
                  </div>
                  <span className="network-card-tag">
                    {hosts.length} 条规则
                  </span>
                </div>
                <div
                  className="network-domain-list"
                  aria-label="已添加的直连域名"
                >
                  {hosts.length ? (
                    hosts.map((host) => (
                      <span className="network-domain-chip" key={host}>
                        <span>{host}</span>
                        <IconButton
                          label={"移除直连域名 " + host}
                          icon={X}
                          disabled={busy || !manual}
                          onClick={() => {
                            setSettings({
                              ...settings,
                              bypass: hosts.filter((h) => h !== host).join(","),
                            });
                            setError("");
                          }}
                        />
                      </span>
                    ))
                  ) : (
                    <span className="network-domain-empty">
                      尚未添加直连域名
                    </span>
                  )}
                </div>
                <div className="network-domain-input">
                  <input
                    aria-label="直连域名"
                    disabled={busy || !manual}
                    value={draft}
                    placeholder="输入域名，按 Enter 添加"
                    spellCheck={false}
                    aria-describedby="network-bypass-help"
                    onChange={(e) => setDraft(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" && !e.nativeEvent.isComposing) {
                        e.preventDefault();
                        addDomains(draft);
                      }
                    }}
                  />
                  <IconButton
                    label="添加直连域名"
                    icon={Plus}
                    disabled={busy || !manual || !draft.trim()}
                    onClick={() => addDomains(draft)}
                  />
                </div>
                <div className="network-domain-hints">
                  <p className="network-field-help" id="network-bypass-help">
                    可粘贴多个域名，用逗号或空格分隔。不填协议、端口或路径。
                  </p>
                  {!hosts.includes(".devops.aliyuncs.com") && (
                    <button
                      className="network-domain-preset"
                      type="button"
                      disabled={busy || !manual}
                      onClick={() =>
                        addDomains([draft, ".devops.aliyuncs.com"].join(","))
                      }
                    >
                      <Plus size={12} aria-hidden="true" />
                      Codeup 直连
                    </button>
                  )}
                </div>
              </section>
              <div className="network-routing-note">
                <Info size={15} aria-hidden="true" />
                <div>
                  <strong>
                    {manual
                      ? "直连规则优先，其余地址使用代理"
                      : "当前沿用已有网络配置"}
                  </strong>
                  <span>适用于 Fetch、同步和克隆。SSH 仓库沿用 SSH 配置。</span>
                </div>
              </div>
            </>
          ) : (
            !error && (
              <div className="network-settings-loading">
                <LoaderCircle size={20} className="spin" aria-hidden="true" />
                正在读取网络设置…
              </div>
            )
          )}
          {error && (
            <div className="network-settings-error" role="alert">
              <Info size={16} aria-hidden="true" />
              <span>{error}</span>
              {!settings && (
                <button type="button" onClick={() => setRetry((n) => n + 1)}>
                  <RefreshCw size={14} aria-hidden="true" />
                  重试
                </button>
              )}
            </div>
          )}
        </div>
        <footer className="network-settings-footer">
          <span className="network-save-note">
            <ShieldCheck size={14} aria-hidden="true" />
            {dirty ? "有未保存的更改" : "设置仅保存在本机"}
          </span>
          <div>
            <button type="button" disabled={busy} onClick={onClose}>
              取消
            </button>
            <button className="primary" disabled={!settings || busy}>
              {busy ? (
                <LoaderCircle size={15} className="spin" aria-hidden="true" />
              ) : (
                <Check size={15} aria-hidden="true" />
              )}
              {busy ? "保存中…" : "保存"}
            </button>
          </div>
        </footer>
      </form>
    </Dialog>
  );
}
