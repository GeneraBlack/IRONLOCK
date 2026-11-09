import Vue from "vue";
import App from "./App.vue";
import router from "./router";
import store from "./store";

import CarbonComponentsVue from "@carbon/vue";
Vue.use(CarbonComponentsVue);

import axios from "axios";
axios.defaults.timeout = 10000;
import VueAxios from "vue-axios";
Vue.use(VueAxios, axios);

import ns8Lib, { Filters } from "@nethserver/ns8-ui-lib";
Vue.use(ns8Lib);

import VueDateFns from "vue-date-fns";
Vue.use(VueDateFns);

import LottieAnimation from "lottie-web-vue";
Vue.use(LottieAnimation);

import vueDebounce from "vue-debounce";
Vue.use(vueDebounce);

for (const f in Filters) {
  Vue.filter(f, Filters[f]);
}

Vue.config.productionTip = false;

import VueI18n from "vue-i18n";
import deMessages from "../public/i18n/de/translation.json";
import enMessages from "../public/i18n/en/translation.json";

Vue.use(VueI18n);

const navigatorLang = (navigator.language || "de").substring(0, 2);
const activeLocale = navigatorLang === "de" ? "de" : "en";

const i18n = new VueI18n({
  locale: activeLocale,
  fallbackLocale: "en",
  messages: {
    de: deMessages,
    en: enMessages,
  },
});

new Vue({
  router,
  store,
  i18n,
  render: (h) => h(App),
}).$mount("#ns8-app");
