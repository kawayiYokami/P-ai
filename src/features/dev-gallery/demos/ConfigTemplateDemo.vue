<template>
  <ConfigTemplate v-model="configTemplateDemo" :groups="configTemplateGroups" />
</template>

<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import ConfigTemplate from "../../config/components/ConfigTemplate.vue";
import type { ConfigTemplateGroup } from "../../config/components/config-template";

const { t } = useI18n();

const configTemplateDemo = ref<Record<string, unknown>>({
  openLocalDocument: true,
  openOnlineDocument: true,
  autoHideAtScreenEdge: true,
  showTaskbarIcon: true,
  closeApp: "tray",
  openWebInSystemBrowser: true,
  homepage: "https://pai.example.com",
  browserNote: "",
});

const configTemplateGroups: ConfigTemplateGroup[] = [
  {
    title: t("config.demo.generalSection"),
    rows: [
      {
        items: [
          {
            key: "openLocalDocument",
            label: t("config.demo.openLocalDocument"),
            description: t("config.demo.openLocalDocumentHint"),
            type: "toggle",
          },
        ],
      },
      {
        items: [
          {
            key: "openOnlineDocument",
            label: t("config.demo.openOnlineDocument"),
            type: "toggle",
          },
        ],
      },
    ],
  },
  {
    title: t("config.demo.mainPanelSection"),
    rows: [
      {
        items: [
          { key: "autoHideAtScreenEdge", label: t("config.demo.autoHideAtScreenEdge"), type: "toggle" },
          { key: "showTaskbarIcon", label: t("config.demo.showTaskbarIcon"), type: "toggle" },
        ],
      },
      {
        items: [
          {
            key: "closeApp",
            label: t("config.demo.closeApp"),
            type: "select",
            options: [
              { value: "tray", label: t("config.demo.closeAppToTray") },
              { value: "quit", label: t("config.demo.closeAppQuit") },
            ],
          },
        ],
      },
    ],
  },
  {
    title: t("config.demo.browseSection"),
    rows: [
      {
        items: [
          { key: "openWebInSystemBrowser", label: t("config.demo.openWebInSystemBrowser"), type: "toggle" },
        ],
      },
      {
        items: [
          {
            key: "homepage",
            label: t("config.demo.homepage"),
            type: "text",
            placeholder: t("config.demo.homepagePlaceholder"),
          },
          {
            key: "browserNote",
            label: t("config.demo.browserNote"),
            type: "textarea",
            placeholder: t("config.demo.browserNotePlaceholder"),
          },
        ],
      },
    ],
  },
];
</script>
