import { Injectable } from "@angular/core";
import { invoke } from "@tauri-apps/api/core";
import { defer, from, map, Observable } from "rxjs";

import { ModelInfo, ModelInfoDto, ProviderHealth, ProviderHealthDto } from "./provider.models";

interface ProviderCommands {
  readonly healthCommand: string;
  readonly listModelsCommand: string;
}

const PROVIDER_COMMANDS: Readonly<Record<string, ProviderCommands>> = {
  ollama: {
    healthCommand: "check_ollama_health",
    listModelsCommand: "list_ollama_models",
  },
};

@Injectable({
  providedIn: "root",
})
export class ProviderService {
  checkProviderHealth(providerId: string): Observable<ProviderHealth> {
    return defer(() => {
      const commands = this.resolveProviderCommands(providerId);
      return from(invoke<ProviderHealthDto>(commands.healthCommand));
    }).pipe(
      map((response) => ({
        provider: response.provider,
        healthy: response.healthy,
        baseUrl: response.base_url,
        message: response.message,
      })),
    );
  }

  listProviderModels(providerId: string): Observable<ModelInfo[]> {
    return defer(() => {
      const commands = this.resolveProviderCommands(providerId);
      return from(invoke<ModelInfoDto[]>(commands.listModelsCommand));
    }).pipe(
      map((models) =>
        models.map((model) => ({
          provider: model.provider,
          id: model.id,
          displayName: model.display_name,
          sizeBytes: model.size_bytes,
          modifiedAt: model.modified_at,
        })),
      ),
    );
  }

  private resolveProviderCommands(providerId: string): ProviderCommands {
    const normalizedProviderId = providerId.trim().toLowerCase();
    const commands = PROVIDER_COMMANDS[normalizedProviderId];
    if (commands === undefined) {
      throw new Error(`Unsupported provider '${providerId}'.`);
    }

    return commands;
  }
}
