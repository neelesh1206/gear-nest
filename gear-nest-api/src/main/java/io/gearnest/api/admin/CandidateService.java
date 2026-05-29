package io.gearnest.api.admin;

import io.gearnest.api.error.NotFoundException;
import io.gearnest.api.product.dto.StoreListing;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.util.List;
import java.util.UUID;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ConcurrentMap;

@Service
public class CandidateService {

    private final CandidateRepository repo;
    private final CandidateReassignWorker worker;
    private final ConcurrentMap<UUID, CompletableFuture<Void>> jobs = new ConcurrentHashMap<>();

    public CandidateService(CandidateRepository repo, CandidateReassignWorker worker) {
        this.repo = repo;
        this.worker = worker;
    }

    public List<StoreListing> listCandidates() {
        return repo.findCandidates();
    }

    @Transactional
    public void confirm(UUID listingId) {
        int rows = repo.confirm(listingId);
        if (rows == 0) {
            throw new NotFoundException("Listing not found: " + listingId);
        }
    }

    public UUID dispatchReassign(UUID listingId, UUID targetProductId) {
        CandidateRepository.ListingPointer pointer = repo.findListing(listingId)
            .orElseThrow(() -> new NotFoundException("Listing not found: " + listingId));
        if (!repo.productExists(targetProductId)) {
            throw new NotFoundException("Product not found: " + targetProductId);
        }
        UUID jobId = UUID.randomUUID();
        jobs.put(jobId, worker.run(listingId, pointer, targetProductId));
        return jobId;
    }

    public CompletableFuture<Void> jobFuture(UUID jobId) {
        return jobs.get(jobId);
    }
}
