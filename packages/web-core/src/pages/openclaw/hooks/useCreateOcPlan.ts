import { useMutation } from '@tanstack/react-query';
import { openclawApi } from '@/shared/lib/api';
import type { CreateOcPlanRequest, CreateOcPlanResponse } from '../oc-types';

export function useCreateOcPlan() {
  const mutation = useMutation<
    CreateOcPlanResponse,
    Error,
    CreateOcPlanRequest
  >({
    mutationFn: (data) => openclawApi.createPlan(data),
  });

  return mutation;
}
