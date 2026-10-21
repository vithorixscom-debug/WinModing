import { invoke } from "@tauri-apps/api/core";

const DEFAULT_KEY = "winmoding.defaultUser";

function toCredential(account) {
  if (!account) return null;
  const kind = account.kind ?? (account.access_token && account.access_token !== "0" ? "microsoft" : "offline");
  return {
    id: account.id,
    username: account.username,
    uuid: account.uuid,
    active: true,
    kind,
    access_token: kind === "offline" ? null : kind,
    profile: {
      name: account.username,
      id: account.uuid,
    },
  };
}

export async function offline_login(name) {
  const account = await invoke("login_offline", { username: name });
  const cred = toCredential(account);
  await set_default_user(cred.id);
  return cred;
}

export async function login() {
  const account = await invoke("login_msa");
  const cred = toCredential(account);
  await set_default_user(cred.id);
  return cred;
}

export async function ely_login(username, password) {
  const account = await invoke("login_ely", { username, password });
  const cred = toCredential(account);
  await set_default_user(cred.id);
  return cred;
}

export async function get_default_user() {
  const stored = localStorage.getItem(DEFAULT_KEY);
  const usersList = await users();
  if (stored && usersList.some((item) => item.id === stored)) return stored;
  return usersList[0]?.id ?? undefined;
}

export async function set_default_user(user) {
  if (user) localStorage.setItem(DEFAULT_KEY, user);
  else localStorage.removeItem(DEFAULT_KEY);
}

export async function remove_user(user) {
  await invoke("remove_account", { id: user });
  if (localStorage.getItem(DEFAULT_KEY) === user) {
    localStorage.removeItem(DEFAULT_KEY);
  }
}

export async function users() {
  const accounts = await invoke("list_accounts");
  return (accounts ?? []).map(toCredential);
}
