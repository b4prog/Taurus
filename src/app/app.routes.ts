import { Routes } from "@angular/router";

import { ChatShellComponent } from "./features/chat/chat-shell.component";
import { SettingsComponent } from "./features/settings/settings.component";

export const routes: Routes = [
  {
    path: "",
    component: ChatShellComponent,
  },
  {
    path: "settings",
    component: SettingsComponent,
  },
  {
    path: "**",
    redirectTo: "",
  },
];
