package io.gearnest.api.admin;

import com.fasterxml.jackson.annotation.JsonProperty;
import io.gearnest.api.product.dto.StoreListing;
import jakarta.validation.Valid;
import jakarta.validation.constraints.NotNull;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

import java.util.List;
import java.util.Map;
import java.util.UUID;

@RestController
@RequestMapping("/api/admin/candidates")
public class AdminCandidateController {

    private final CandidateService service;

    public AdminCandidateController(CandidateService service) {
        this.service = service;
    }

    @GetMapping
    public List<StoreListing> list() {
        return service.listCandidates();
    }

    @PostMapping("/{listingId}/confirm")
    public ResponseEntity<Void> confirm(@PathVariable UUID listingId) {
        service.confirm(listingId);
        return ResponseEntity.noContent().build();
    }

    @PostMapping("/{listingId}/reassign")
    public ResponseEntity<Map<String, String>> reassign(
        @PathVariable UUID listingId,
        @Valid @RequestBody ReassignRequest body
    ) {
        UUID jobId = service.dispatchReassign(listingId, body.targetProductId());
        return ResponseEntity.accepted().body(Map.of("jobId", jobId.toString()));
    }

    public record ReassignRequest(
        @JsonProperty("target_product_id") @NotNull UUID targetProductId
    ) {}
}
