import { Injectable } from "@angular/core";
import { invoke } from "@tauri-apps/api/core";
import { from, map, Observable } from "rxjs";

import { ModelInfo, ModelInfoDto, ProviderHealth, ProviderHealthDto } from "./provider.models";

@Injectable({
  providedIn: "root",
})
export class ProviderService {
  checkOllamaHealth(): Observable<ProviderHealth> {
    return from(invoke<ProviderHealthDto>("check_ollama_health")).pipe(
      map((response) => ({
        provider: response.provider,
        healthy: response.healthy,
        baseUrl: response.base_url,
        message: response.message,
      })),
    );
  }

  listOllamaModels(): Observable<ModelInfo[]> {
    return from(invoke<ModelInfoDto[]>("list_ollama_models")).pipe(
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
}
