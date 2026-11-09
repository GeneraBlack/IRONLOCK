<template>
  <div id="ns8-app">
    <cv-content id="main-content" class="app-content">
      <AppSideMenu />
      <AppMobileSideMenu />
      <router-view />
    </cv-content>
  </div>
</template>

<script>
import AppSideMenu from "./components/AppSideMenu";
import AppMobileSideMenu from "./components/AppMobileSideMenu";
import { mapState, mapActions } from "vuex";
import {
  QueryParamService,
  TaskService,
  UtilService,
} from "@nethserver/ns8-ui-lib";
import to from "await-to-js";

export default {
  name: "App",
  components: { AppSideMenu, AppMobileSideMenu },
  mixins: [QueryParamService, TaskService, UtilService],
  data() {
    return {
      parentUrlSyncInterval: null,
      lastParentHash: "",
    };
  },
  computed: {
    ...mapState(["instanceName", "instanceLabel", "core"]),
  },
  created() {
    const core = window.parent.core;
    this.setCoreInStore(core);
    const hashMatch = /#\/apps\/([a-zA-Z0-9_-]+)/.exec(
      window.parent.location.hash
    );
    const instanceName = hashMatch ? hashMatch[1] : "rust-rts1";
    this.setInstanceNameInStore(instanceName);
    this.getInstanceLabel();
    this.setAppName();

    // Listen to changeRoute events from ns8-ui-lib or side menu
    const context = this;
    window.addEventListener(
      "changeRoute",
      function (e) {
        const requestedPage = String(e.detail || "status").replace(/^\//, "");
        context.navigateInternalRoute(requestedPage);
      },
      false
    );

    // Configure global shortcuts
    core.$root.$emit("configureKeyboardShortcuts", window);

    const queryParams = this.getQueryParamsForApp();
    const requestedPage = queryParams.page || "status";

    if (requestedPage != "status") {
      this.navigateInternalRoute(requestedPage);
    }

    // Keep internal Vue Router synchronized with parent NS8 URL hash at all times
    this.lastParentHash = window.parent.location.hash;
    this.parentUrlSyncInterval = setInterval(() => {
      const currentHash = window.parent.location.hash;
      if (currentHash !== this.lastParentHash) {
        this.lastParentHash = currentHash;
        const params = this.getQueryParamsForApp();
        const page = params.page || "status";
        this.navigateInternalRoute(page);
        this.$root.$emit("appNavigation", this.instanceName, page);
      }
    }, 250);
  },
  beforeDestroy() {
    if (this.parentUrlSyncInterval) {
      clearInterval(this.parentUrlSyncInterval);
    }
  },
  methods: {
    ...mapActions([
      "setInstanceNameInStore",
      "setInstanceLabelInStore",
      "setCoreInStore",
      "setAppNameInStore",
    ]),
    navigateInternalRoute(page) {
      const cleanPage = String(page || "status").replace(/^\//, "");
      const targetPath = "/" + cleanPage;
      const currentPath = this.$route ? this.$route.path : "";
      if (
        currentPath !== targetPath &&
        !(cleanPage === "status" && currentPath === "/")
      ) {
        this.$router.replace(targetPath).catch(() => {});
      }
    },
    async getInstanceLabel() {
      const taskAction = "get-name";
      const eventId = this.getUuid();

      this.core.$root.$once(
        `${taskAction}-aborted-${eventId}`,
        this.getInstanceLabelAborted
      );
      this.core.$root.$once(
        `${taskAction}-completed-${eventId}`,
        this.getInstanceLabelCompleted
      );

      const res = await to(
        this.createModuleTaskForApp(this.instanceName, {
          action: taskAction,
          extra: {
            title: this.$t("action." + taskAction),
            isNotificationHidden: true,
            eventId,
          },
        })
      );
      const err = res[0];

      if (err) {
        console.error(`error creating task ${taskAction}`, err);
        this.createErrorNotificationForApp(
          err,
          this.$t("task.cannot_create_task", { action: taskAction })
        );
        return;
      }
    },
    getInstanceLabelAborted(taskResult, taskContext) {
      console.error(`${taskContext.action} aborted`, taskResult);
    },
    getInstanceLabelCompleted(taskContext, taskResult) {
      this.setInstanceLabelInStore(taskResult.output.name);
    },
    setAppName() {
      const metadata = require("../public/metadata.json");
      const appName = metadata.name;
      this.setAppNameInStore(appName);
    },
  },
};
</script>

<style lang="scss">
@import "styles/carbon-utils";
</style>
