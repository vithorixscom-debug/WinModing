import { defineStore } from "pinia";

export const useNotifications = defineStore("notificationsStore", {
  state: () => ({
    notificationsWrapper: null,
  }),
  actions: {
    setNotifs(notifs) {
      this.notificationsWrapper = notifs;
    },
    addNotification(notif) {
      if (this.notificationsWrapper?.addNotification) {
        this.notificationsWrapper.addNotification(notif);
        return;
      }
      console.warn("[WinModing]", notif?.title, notif?.text ?? notif);
    },
  },
});

export const handleError = (err) => {
  const message = err?.message ?? String(err ?? "Unknown error");
  try {
    const notifs = useNotifications();
    notifs.addNotification({
      title: "An error occurred",
      text: message,
      type: "error",
    });
  } catch (nested) {
    console.error(nested);
  }
  console.error(err);
};
