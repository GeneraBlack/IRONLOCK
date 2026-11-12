<template>
  <cv-grid fullWidth>
    <cv-row>
      <cv-column class="page-title">
        <h2>{{ $t("settings.title") }}</h2>
      </cv-column>
    </cv-row>
    <cv-row v-if="error.getConfiguration">
      <cv-column>
        <NsInlineNotification
          kind="error"
          :title="$t('action.get-configuration')"
          :description="error.getConfiguration"
          :showCloseButton="false"
        />
      </cv-column>
    </cv-row>
    <cv-row>
      <cv-column>
        <cv-tile light>
          <cv-form @submit.prevent="configureModule">
            <cv-text-input
              :label="$t('settings.host')"
              placeholder="rust-rts.bls-isp.net"
              v-model.trim="host"
              class="mg-bottom-md"
              :invalid-message="error.host"
              :disabled="loading.getConfiguration || loading.configureModule"
              ref="host"
            />
            <cv-toggle
              value="letsEncrypt"
              :label="$t('settings.lets_encrypt')"
              v-model="isLetsEncryptEnabled"
              :disabled="loading.getConfiguration || loading.configureModule"
              class="mg-bottom-md"
            >
              <template slot="text-left">{{
                $t("settings.disabled")
              }}</template>
              <template slot="text-right">{{
                $t("settings.enabled")
              }}</template>
            </cv-toggle>
            <cv-toggle
              value="httpToHttps"
              :label="$t('settings.http2https')"
              v-model="isHttpToHttpsEnabled"
              :disabled="loading.getConfiguration || loading.configureModule"
              class="mg-bottom-md"
            >
              <template slot="text-left">{{
                $t("settings.disabled")
              }}</template>
              <template slot="text-right">{{
                $t("settings.enabled")
              }}</template>
            </cv-toggle>

            <cv-accordion ref="accordion" class="maxwidth mg-bottom-md">
              <cv-accordion-item :open="toggleAccordion[0]">
                <template slot="title">{{ $t("settings.advanced") }}</template>
                <template slot="content">
                  <cv-text-input
                    :label="$t('settings.ttl_secs')"
                    type="number"
                    v-model.trim="ttlSecs"
                    :invalid-message="error.ttl_secs"
                    class="mg-bottom-md"
                    :disabled="
                      loading.getConfiguration || loading.configureModule
                    "
                    ref="ttl_secs"
                  />
                  <cv-text-input
                    :label="$t('settings.node_id')"
                    placeholder="bls-isp-eu-central-01"
                    v-model.trim="rtsNodeId"
                    :invalid-message="error.rts_node_id"
                    class="mg-bottom-md"
                    :disabled="
                      loading.getConfiguration || loading.configureModule
                    "
                    ref="rts_node_id"
                  />
                  <cv-text-input
                    :label="$t('settings.relay_port_start')"
                    type="number"
                    v-model.trim="relayPortStart"
                    :invalid-message="error.relay_port_start"
                    class="mg-bottom-md"
                    :disabled="
                      loading.getConfiguration || loading.configureModule
                    "
                    ref="relay_port_start"
                  />
                  <cv-text-input
                    :label="$t('settings.relay_port_end')"
                    type="number"
                    v-model.trim="relayPortEnd"
                    :invalid-message="error.relay_port_end"
                    class="mg-bottom-md"
                    :disabled="
                      loading.getConfiguration || loading.configureModule
                    "
                    ref="relay_port_end"
                  />
                </template>
              </cv-accordion-item>
            </cv-accordion>

            <cv-row v-if="error.configureModule">
              <cv-column>
                <NsInlineNotification
                  kind="error"
                  :title="$t('action.configure-module')"
                  :description="error.configureModule"
                  :showCloseButton="false"
                />
              </cv-column>
            </cv-row>
            <NsButton
              kind="primary"
              :icon="Save20"
              :loading="loading.configureModule"
              :disabled="loading.getConfiguration || loading.configureModule"
              >{{ $t("settings.save") }}</NsButton
            >
          </cv-form>
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
  UtilService,
  TaskService,
  IconService,
  PageTitleService,
} from "@nethserver/ns8-ui-lib";

export default {
  name: "Settings",
  mixins: [
    TaskService,
    IconService,
    UtilService,
    QueryParamService,
    PageTitleService,
  ],
  pageTitle() {
    return this.$t("settings.title") + " - " + this.appName;
  },
  data() {
    return {
      q: {
        page: "settings",
      },
      urlCheckInterval: null,
      toggleAccordion: [false],
      host: "",
      isLetsEncryptEnabled: false,
      isHttpToHttpsEnabled: false,
      ttlSecs: "45",
      rtsNodeId: "bls-isp-eu-central-01",
      relayPortStart: "30100",
      relayPortEnd: "30139",
      loading: {
        getConfiguration: false,
        configureModule: false,
      },
      error: {
        getConfiguration: "",
        configureModule: "",
        host: "",
        ttl_secs: "",
        rts_node_id: "",
        relay_port_start: "",
        relay_port_end: "",
      },
    };
  },
  computed: {
    ...mapState(["instanceName", "core", "appName"]),
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
  beforeDestroy() {
    clearInterval(this.urlCheckInterval);
  },
  created() {
    this.getConfiguration();
  },
  methods: {
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
      this.isLetsEncryptEnabled = Boolean(config.lets_encrypt);
      this.isHttpToHttpsEnabled = Boolean(config.http2https);
      this.ttlSecs = String(config.ttl_secs || 45);
      this.rtsNodeId =
        config.rts_node_id || config.node_id || "bls-isp-eu-central-01";
      this.relayPortStart = String(config.relay_port_start || 30100);
      this.relayPortEnd = String(config.relay_port_end || 30139);
      this.loading.getConfiguration = false;
      this.focusElement("host");
    },
    validateConfigureModule() {
      this.clearErrors(this);
      let isValidationOk = true;

      if (!this.host) {
        this.error.host = this.$t("common.required");
        if (isValidationOk) {
          this.focusElement("host");
          isValidationOk = false;
        }
      }

      const startPort = Number(this.relayPortStart);
      const endPort = Number(this.relayPortEnd);
      if (!startPort || startPort < 1024 || startPort > 65535) {
        this.error.relay_port_start = this.$t("error.validation_error");
        this.toggleAccordion[0] = true;
        isValidationOk = false;
      }
      if (!endPort || endPort < startPort || endPort > 65535) {
        this.error.relay_port_end = this.$t("error.validation_error");
        this.toggleAccordion[0] = true;
        isValidationOk = false;
      }

      return isValidationOk;
    },
    configureModuleValidationFailed(validationErrors) {
      this.loading.configureModule = false;
      let focusAlreadySet = false;

      for (const validationError of validationErrors) {
        const field = validationError.field;
        if (field !== "(root)") {
          if (this.$te("error." + validationError.error)) {
            this.error[field] = this.$t("error." + validationError.error);
          } else if (this.$te("settings." + validationError.error)) {
            this.error[field] = this.$t("settings." + validationError.error);
          } else {
            this.error[field] = this.$t("error.validation_error");
          }
          if (!focusAlreadySet) {
            this.focusElement(field);
            focusAlreadySet = true;
          }
        }
      }
    },
    async configureModule() {
      const isValidationOk = this.validateConfigureModule();
      if (!isValidationOk) {
        return;
      }

      this.loading.configureModule = true;
      const taskAction = "configure-module";
      const eventId = this.getUuid();

      this.core.$root.$once(
        `${taskAction}-aborted-${eventId}`,
        this.configureModuleAborted
      );
      this.core.$root.$once(
        `${taskAction}-validation-failed-${eventId}`,
        this.configureModuleValidationFailed
      );
      this.core.$root.$once(
        `${taskAction}-completed-${eventId}`,
        this.configureModuleCompleted
      );

      const res = await to(
        this.createModuleTaskForApp(this.instanceName, {
          action: taskAction,
          data: {
            host: this.host,
            lets_encrypt: this.isLetsEncryptEnabled,
            http2https: this.isHttpToHttpsEnabled,
            ttl_secs: Number(this.ttlSecs) || 45,
            rts_node_id: this.rtsNodeId || "bls-isp-eu-central-01",
            relay_port_start: Number(this.relayPortStart) || 30100,
            relay_port_end: Number(this.relayPortEnd) || 30139,
          },
          extra: {
            title: this.$t("settings.configure_instance", {
              instance: this.instanceName,
            }),
            description: this.$t("common.processing"),
            eventId,
          },
        })
      );
      const err = res[0];

      if (err) {
        console.error(`error creating task ${taskAction}`, err);
        this.error.configureModule = this.getErrorMessage(err);
        this.loading.configureModule = false;
        return;
      }
    },
    configureModuleAborted(taskResult, taskContext) {
      console.error(`${taskContext.action} aborted`, taskResult);
      this.error.configureModule = this.$t("error.generic_error");
      this.loading.configureModule = false;
    },
    configureModuleCompleted() {
      this.loading.configureModule = false;
      this.getConfiguration();
    },
  },
};
</script>

<style scoped lang="scss">
@import "../styles/carbon-utils";
</style>
