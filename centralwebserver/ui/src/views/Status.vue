<template>
  <cv-grid fullWidth>
    <cv-row>
      <cv-column class="page-title">
        <h2>{{ $t("status.title") }}</h2>
      </cv-column>
    </cv-row>
    <cv-row v-if="error.getStatus">
      <cv-column>
        <NsInlineNotification
          kind="error"
          :title="$t('action.get-status')"
          :description="error.getStatus"
          :showCloseButton="false"
        />
      </cv-column>
    </cv-row>
    <cv-row v-if="error.listBackupRepositories">
      <cv-column>
        <NsInlineNotification
          kind="error"
          :title="$t('action.list-backup-repositories')"
          :description="error.listBackupRepositories"
          :showCloseButton="false"
        />
      </cv-column>
    </cv-row>
    <cv-row v-if="error.listBackups">
      <cv-column>
        <NsInlineNotification
          kind="error"
          :title="$t('action.list-backups')"
          :description="error.listBackups"
          :showCloseButton="false"
        />
      </cv-column>
    </cv-row>
    <cv-row>
      <cv-column :md="4" :max="4">
        <NsInfoCard
          light
          :title="$t('status.webapp')"
          :description="host ? host : $t('status.not_configured')"
          :icon="Wikis32"
          :loading="loading.getConfiguration"
          :isErrorShown="Boolean(error.getConfiguration)"
          :errorTitle="$t('error.cannot_retrieve_configuration')"
          :errorDescription="error.getConfiguration"
          class="min-height-card"
        >
          <template slot="content">
            <NsButton
              v-if="host"
              kind="ghost"
              :icon="Launch20"
              :disabled="loading.getConfiguration"
              @click="goToWebapp"
            >
              {{ $t("status.open_webapp") }}
            </NsButton>
            <NsButton
              v-else
              kind="ghost"
              :disabled="loading.getConfiguration"
              :icon="ArrowRight20"
              @click="goToSettings"
            >
              {{ $t("status.configure") }}
            </NsButton>
          </template>
        </NsInfoCard>
      </cv-column>
      <cv-column :md="4" :max="4">
        <NsInfoCard
          light
          :title="status.instance || '-'"
          :description="serviceRuntimeStatusLabel"
          :icon="Application32"
          :loading="loading.getStatus"
          class="min-height-card"
        />
      </cv-column>
      <cv-column :md="4" :max="4">
        <NsInfoCard
          light
          :title="installationNodeTitle"
          :titleTooltip="installationNodeTitleTooltip"
          :description="$t('status.installation_node')"
          :icon="Chip32"
          :loading="loading.getStatus"
          class="min-height-card"
        />
      </cv-column>
      <cv-column :md="4" :max="4">
        <NsBackupCard
          :title="core.$t('backup.title')"
          :noBackupMessage="core.$t('backup.no_backup_configured')"
          :goToBackupLabel="core.$t('backup.go_to_backup')"
          :repositoryLabel="core.$t('backup.repository')"
          :statusLabel="core.$t('common.status')"
          :statusSuccessLabel="core.$t('common.success')"
          :statusNotRunLabel="core.$t('backup.backup_has_not_run_yet')"
          :statusErrorLabel="core.$t('error.error')"
          :completedLabel="core.$t('backup.completed')"
          :durationLabel="core.$t('backup.duration')"
          :totalSizeLabel="core.$t('backup.total_size')"
          :totalFileCountLabel="core.$t('backup.total_file_count')"
          :backupDisabledLabel="core.$t('common.disabled')"
          :showMoreLabel="core.$t('common.show_more')"
          :multipleUncertainStatusLabel="
            core.$t('backup.some_backups_failed_or_are_pending')
          "
          :moduleId="instanceName"
          :moduleUiName="instanceLabel"
          :repositories="backupRepositories"
          :backups="backups"
          :loading="loading.listBackupRepositories || loading.listBackups"
          :coreContext="core"
          light
        />
      </cv-column>
      <cv-column :md="4" :max="4">
        <NsSystemLogsCard
          :title="core.$t('system_logs.card_title')"
          :description="
            core.$t('system_logs.card_description', {
              name: instanceLabel || instanceName,
            })
          "
          :buttonLabel="core.$t('system_logs.card_button_label')"
          :router="core.$router"
          context="module"
          :moduleId="instanceName"
          light
        />
      </cv-column>
      <cv-column :md="4" :max="4">
        <NsInfoCard
          light
          :title="$t('status.module_update_title')"
          :description="
            $t('status.current_version') + ': ' + (installedVersion || '-')
          "
          :icon="Application32"
          :loading="loading.listInstalledModules"
          :isErrorShown="Boolean(error.updateModule)"
          :errorTitle="$t('action.update-module')"
          :errorDescription="error.updateModule"
          class="min-height-card"
        >
          <template slot="content">
            <div class="module-update-controls">
              <cv-text-input
                :label="$t('status.target_version')"
                placeholder="1.0.1"
                v-model.trim="targetUpdateVersion"
                :invalid-message="error.targetVersion"
                :disabled="loading.updateModule || !moduleSource"
                class="mg-bottom-sm"
              />
              <NsButton
                kind="secondary"
                size="field"
                :icon="ArrowRight20"
                :loading="loading.updateModule"
                :disabled="
                  loading.updateModule ||
                  loading.listInstalledModules ||
                  !moduleSource
                "
                @click="triggerModuleUpdate"
              >
                {{ $t("status.update_now") }}
              </NsButton>
            </div>
          </template>
        </NsInfoCard>
      </cv-column>
    </cv-row>
    <!-- services -->
    <cv-row>
      <cv-column class="page-subtitle">
        <h4>{{ $tc("status.services", 2) }}</h4>
      </cv-column>
    </cv-row>
    <cv-row v-if="!loading.getStatus">
      <cv-column v-if="!status.services || !status.services.length">
        <cv-tile light>
          <NsEmptyState :title="$t('status.no_services')" />
        </cv-tile>
      </cv-column>
      <cv-column
        v-else
        v-for="(service, index) in status.services"
        :key="index"
        :md="4"
        :max="4"
      >
        <NsSystemdServiceCard
          light
          class="min-height-card"
          :serviceName="service.name"
          :active="service.active"
          :failed="service.failed"
          :enabled="service.enabled"
          :icon="Cube32"
        />
      </cv-column>
    </cv-row>
    <cv-row v-else>
      <cv-column :md="4" :max="4">
        <cv-tile light>
          <cv-skeleton-text :paragraph="true" :line-count="4" />
        </cv-tile>
      </cv-column>
    </cv-row>
    <!-- images -->
    <cv-row>
      <cv-column class="page-subtitle">
        <h4>{{ $tc("status.app_images", 2) }}</h4>
      </cv-column>
    </cv-row>
    <cv-row>
      <cv-column>
        <cv-tile light>
          <div v-if="!loading.getStatus">
            <NsEmptyState
              v-if="!status.images || !status.images.length"
              :title="$t('status.no_images')"
            />
            <cv-structured-list v-else>
              <template slot="headings">
                <cv-structured-list-heading>{{
                  $t("status.name")
                }}</cv-structured-list-heading>
                <cv-structured-list-heading>{{
                  $t("status.size")
                }}</cv-structured-list-heading>
                <cv-structured-list-heading>{{
                  $t("status.created")
                }}</cv-structured-list-heading>
              </template>
              <template slot="items">
                <cv-structured-list-item
                  v-for="(image, index) in status.images"
                  :key="index"
                >
                  <cv-structured-list-data class="break-word">{{
                    image.name
                  }}</cv-structured-list-data>
                  <cv-structured-list-data>{{
                    formatImageSize(image.size)
                  }}</cv-structured-list-data>
                  <cv-structured-list-data class="break-word">{{
                    image.created
                  }}</cv-structured-list-data>
                </cv-structured-list-item>
              </template>
            </cv-structured-list>
          </div>
          <cv-skeleton-text v-else :paragraph="true" :line-count="5" />
        </cv-tile>
      </cv-column>
    </cv-row>
    <!-- volumes -->
    <cv-row>
      <cv-column class="page-subtitle">
        <h4>{{ $tc("status.app_volumes", 2) }}</h4>
      </cv-column>
    </cv-row>
    <cv-row>
      <cv-column>
        <cv-tile light>
          <div v-if="!loading.getStatus">
            <NsEmptyState
              v-if="!status.volumes || !status.volumes.length"
              :title="$t('status.no_volumes')"
            />
            <cv-structured-list v-else>
              <template slot="headings">
                <cv-structured-list-heading>{{
                  $t("status.name")
                }}</cv-structured-list-heading>
                <cv-structured-list-heading>{{
                  $t("status.mount")
                }}</cv-structured-list-heading>
                <cv-structured-list-heading>{{
                  $t("status.created")
                }}</cv-structured-list-heading>
              </template>
              <template slot="items">
                <cv-structured-list-item
                  v-for="(volume, index) in status.volumes"
                  :key="index"
                >
                  <cv-structured-list-data>{{
                    volume.name
                  }}</cv-structured-list-data>
                  <cv-structured-list-data class="break-word">{{
                    volume.mount
                  }}</cv-structured-list-data>
                  <cv-structured-list-data>{{
                    volume.created
                  }}</cv-structured-list-data>
                </cv-structured-list-item>
              </template>
            </cv-structured-list>
          </div>
          <cv-skeleton-text v-else :paragraph="true" :line-count="5" />
        </cv-tile>
      </cv-column>
    </cv-row>
  </cv-grid>
</template>

<script>
import to from "await-to-js";
import { mapState } from "vuex";
import {
  QueryParamService,
  TaskService,
  IconService,
  UtilService,
  PageTitleService,
} from "@nethserver/ns8-ui-lib";

export default {
  name: "Status",
  mixins: [
    TaskService,
    QueryParamService,
    IconService,
    UtilService,
    PageTitleService,
  ],
  pageTitle() {
    return this.$t("status.title") + " - " + this.appName;
  },
  data() {
    return {
      q: {
        page: "status",
      },
      urlCheckInterval: null,
      isRedirectChecked: false,
      redirectTimeout: 0,
      host: "",
      status: {
        instance: "",
        services: [],
        images: [],
        volumes: [],
      },
      backupRepositories: [],
      backups: [],
      installedVersion: "-",
      moduleSource: "",
      targetUpdateVersion: "",
      loading: {
        getStatus: false,
        getConfiguration: false,
        listBackupRepositories: false,
        listBackups: false,
        listInstalledModules: false,
        updateModule: false,
      },
      error: {
        getStatus: "",
        getConfiguration: "",
        listBackupRepositories: "",
        listBackups: "",
        updateModule: "",
        targetVersion: "",
      },
    };
  },
  computed: {
    ...mapState(["instanceName", "instanceLabel", "core", "appName"]),
    serviceRuntimeStatusLabel() {
      const base = this.$t("status.app_instance");
      if (this.loading.getStatus) {
        return base;
      }
      const services = (this.status && this.status.services) || [];
      if (!services.length) {
        return `${base} · ${this.$t("status.no_services")}`;
      }
      if (services.some((s) => Boolean(s.failed))) {
        return `${base} · ${this.$t("status.service_failed")}`;
      }
      if (services.every((s) => Boolean(s.active))) {
        return `${base} · ${this.$t("status.service_running")}`;
      }
      return `${base} · ${this.$t("status.service_inactive")}`;
    },
    installationNodeTitle() {
      if (this.status && this.status.node) {
        if (this.status.node_ui_name) {
          return this.status.node_ui_name;
        } else {
          return this.$t("status.node") + " " + this.status.node;
        }
      } else {
        return "-";
      }
    },
    installationNodeTitleTooltip() {
      if (this.status && this.status.node_ui_name) {
        return this.$t("status.node") + " " + this.status.node;
      } else {
        return "";
      }
    },
  },
  beforeRouteEnter(to, from, next) {
    next((vm) => {
      vm.watchQueryData(vm);
      vm.urlCheckInterval = vm.initUrlBindingForApp(vm, vm.q.page);
    });
  },
  beforeRouteLeave(to, from, next) {
    clearInterval(this.urlCheckInterval);
    next();
  },
  mounted() {
    this.redirectTimeout = setTimeout(
      () => (this.isRedirectChecked = true),
      200
    );
  },
  beforeDestroy() {
    clearTimeout(this.redirectTimeout);
    clearInterval(this.urlCheckInterval);
  },
  created() {
    this.getStatus();
    this.getConfiguration();
    this.listBackupRepositories();
    this.listInstalledModules();
  },
  methods: {
    formatImageSize(rawSize) {
      if (rawSize === null || rawSize === undefined || rawSize === "") {
        return "-";
      }
      const numericBytes = Number(rawSize);
      if (!Number.isFinite(numericBytes) || numericBytes < 0) {
        return String(rawSize);
      }
      if (
        this.$options &&
        this.$options.filters &&
        typeof this.$options.filters.byteFormat === "function"
      ) {
        return this.$options.filters.byteFormat(numericBytes);
      }
      if (numericBytes < 1024) {
        return `${Math.round(numericBytes)} B`;
      }
      const units = ["KiB", "MiB", "GiB", "TiB"];
      let value = numericBytes;
      let unitIndex = -1;
      while (value >= 1024 && unitIndex < units.length - 1) {
        value /= 1024;
        unitIndex += 1;
      }
      return `${value.toFixed(value >= 100 ? 0 : 1)} ${units[unitIndex]}`;
    },
    async listInstalledModules() {
      this.loading.listInstalledModules = true;
      const taskAction = "list-installed-modules";
      const eventId = this.getUuid();

      this.core.$root.$once(
        `${taskAction}-aborted-${eventId}`,
        this.listInstalledModulesAborted
      );
      this.core.$root.$once(
        `${taskAction}-completed-${eventId}`,
        this.listInstalledModulesCompleted
      );

      const res = await to(
        this.createClusterTaskForApp({
          action: taskAction,
          extra: {
            title: this.$t("action." + taskAction),
            isNotificationHidden: true,
            eventId,
          },
        })
      );
      if (res[0]) {
        this.loading.listInstalledModules = false;
      }
    },
    listInstalledModulesAborted() {
      this.loading.listInstalledModules = false;
    },
    listInstalledModulesCompleted(taskContext, taskResult) {
      const output = (taskResult && taskResult.output) || {};
      for (const instances of Object.values(output)) {
        if (!Array.isArray(instances)) continue;
        for (const inst of instances) {
          if (inst && inst.id === this.instanceName) {
            this.installedVersion = inst.version || "-";
            this.moduleSource = inst.source || "";
            if (!this.targetUpdateVersion && inst.version) {
              this.targetUpdateVersion = inst.version;
            }
          }
        }
      }
      this.loading.listInstalledModules = false;
    },
    async triggerModuleUpdate() {
      this.error.targetVersion = "";
      this.error.updateModule = "";
      const semverRe =
        /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?$/;
      if (
        !this.targetUpdateVersion ||
        this.targetUpdateVersion.toLowerCase() === "latest" ||
        !semverRe.test(this.targetUpdateVersion)
      ) {
        this.error.targetVersion = this.$t("error.invalid_semver");
        return;
      }
      if (!this.moduleSource) {
        return;
      }

      this.loading.updateModule = true;
      const taskAction = "update-module";
      const eventId = this.getUuid();

      this.core.$root.$once(
        `${taskAction}-aborted-${eventId}`,
        this.updateModuleAborted
      );
      this.core.$root.$once(
        `${taskAction}-completed-${eventId}`,
        this.updateModuleCompleted
      );

      const res = await to(
        this.createClusterTaskForApp({
          action: taskAction,
          data: {
            module_url: `${this.moduleSource}:${this.targetUpdateVersion}`,
            instances: [this.instanceName],
            force: false,
          },
          extra: {
            title: this.$t("action." + taskAction),
            description: this.$t("common.processing"),
            eventId,
          },
        })
      );
      if (res[0]) {
        this.error.updateModule = this.getErrorMessage(res[0]);
        this.loading.updateModule = false;
      }
    },
    updateModuleAborted(taskResult) {
      console.error("update-module aborted", taskResult);
      this.error.updateModule = this.$t("error.generic_error");
      this.loading.updateModule = false;
    },
    updateModuleCompleted() {
      this.loading.updateModule = false;
      this.getStatus();
      this.listInstalledModules();
    },
    goToWebapp() {
      window.open(`https://${this.host}`, "_blank");
    },
    goToSettings() {
      this.goToAppPage(this.instanceName, "settings");
      window.dispatchEvent(
        new CustomEvent("changeRoute", {
          detail: "settings",
        })
      );
      this.$root.$emit("appNavigation", this.instanceName, "settings");
    },
    async getConfiguration() {
      this.loading.getConfiguration = true;
      this.error.getConfiguration = "";
      const taskAction = "get-configuration";
      const eventId = this.getUuid();

      this.core.$root.$once(
        `${taskAction}-aborted-${eventId}`,
        this.getConfigurationAborted
      );
      this.core.$root.$once(
        `${taskAction}-completed-${eventId}`,
        this.getConfigurationCompleted
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
        this.error.getConfiguration = this.getErrorMessage(err);
        this.loading.getConfiguration = false;
        return;
      }
    },
    getConfigurationAborted(taskResult, taskContext) {
      console.error(`${taskContext.action} aborted`, taskResult);
      this.error.getConfiguration = this.$t("error.generic_error");
      this.loading.getConfiguration = false;
    },
    getConfigurationCompleted(taskContext, taskResult) {
      const config = taskResult.output;
      this.host = config.host || "";
      this.loading.getConfiguration = false;
    },
    async getStatus() {
      this.loading.getStatus = true;
      this.error.getStatus = "";
      const taskAction = "get-status";
      const eventId = this.getUuid();

      this.core.$root.$once(
        `${taskAction}-aborted-${eventId}`,
        this.getStatusAborted
      );
      this.core.$root.$once(
        `${taskAction}-completed-${eventId}`,
        this.getStatusCompleted
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
        this.error.getStatus = this.getErrorMessage(err);
        this.loading.getStatus = false;
        return;
      }
    },
    getStatusAborted(taskResult, taskContext) {
      console.error(`${taskContext.action} aborted`, taskResult);
      this.error.getStatus = this.$t("error.generic_error");
      this.loading.getStatus = false;
    },
    getStatusCompleted(taskContext, taskResult) {
      this.status = taskResult.output;
      this.loading.getStatus = false;
    },
    async listBackupRepositories() {
      this.loading.listBackupRepositories = true;
      this.error.listBackupRepositories = "";
      const taskAction = "list-backup-repositories";
      const eventId = this.getUuid();

      this.core.$root.$once(
        `${taskAction}-aborted-${eventId}`,
        this.listBackupRepositoriesAborted
      );
      this.core.$root.$once(
        `${taskAction}-completed-${eventId}`,
        this.listBackupRepositoriesCompleted
      );

      const res = await to(
        this.createClusterTaskForApp({
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
        this.error.listBackupRepositories = this.getErrorMessage(err);
        this.loading.listBackupRepositories = false;
        return;
      }
    },
    listBackupRepositoriesAborted(taskResult, taskContext) {
      console.error(`${taskContext.action} aborted`, taskResult);
      this.error.listBackupRepositories = this.$t("error.generic_error");
      this.loading.listBackupRepositories = false;
    },
    listBackupRepositoriesCompleted(taskContext, taskResult) {
      let backupRepositories = taskResult.output.repositories.sort(
        this.sortByProperty("name")
      );
      this.backupRepositories = backupRepositories;
      this.loading.listBackupRepositories = false;
      this.listBackups();
    },
    async listBackups() {
      this.loading.listBackups = true;
      this.error.listBackups = "";
      const taskAction = "list-backups";
      const eventId = this.getUuid();

      this.core.$root.$once(
        `${taskAction}-aborted-${eventId}`,
        this.listBackupsAborted
      );
      this.core.$root.$once(
        `${taskAction}-completed-${eventId}`,
        this.listBackupsCompleted
      );

      const res = await to(
        this.createClusterTaskForApp({
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
        this.error.listBackups = this.getErrorMessage(err);
        this.loading.listBackups = false;
        return;
      }
    },
    listBackupsAborted(taskResult, taskContext) {
      console.error(`${taskContext.action} aborted`, taskResult);
      this.error.listBackups = this.$t("error.generic_error");
      this.loading.listBackups = false;
    },
    listBackupsCompleted(taskContext, taskResult) {
      let backups = taskResult.output.backups;
      backups.sort(this.sortByProperty("name"));

      for (const backup of backups) {
        const repo = this.backupRepositories.find(
          (r) => r.id == backup.repository
        );

        if (repo) {
          backup.repoName = repo.name;
        }
      }
      this.backups = backups;
      this.loading.listBackups = false;
    },
  },
};
</script>

<style scoped lang="scss">
@import "../styles/carbon-utils";

.break-word {
  word-wrap: break-word;
  max-width: 30vw;
}
</style>
