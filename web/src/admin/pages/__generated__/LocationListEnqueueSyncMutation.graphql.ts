/**
 * @generated SignedSource<<f7f4d65442e1a88d2484072b4f415679>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type LocationListEnqueueSyncMutation$variables = {
  locationId: string;
};
export type LocationListEnqueueSyncMutation$data = {
  readonly enqueueMemberSync: boolean;
};
export type LocationListEnqueueSyncMutation = {
  response: LocationListEnqueueSyncMutation$data;
  variables: LocationListEnqueueSyncMutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "locationId"
  }
],
v1 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "locationId",
        "variableName": "locationId"
      }
    ],
    "kind": "ScalarField",
    "name": "enqueueMemberSync",
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Fragment",
    "metadata": null,
    "name": "LocationListEnqueueSyncMutation",
    "selections": (v1/*:: as any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Operation",
    "name": "LocationListEnqueueSyncMutation",
    "selections": (v1/*:: as any*/)
  },
  "params": {
    "cacheID": "34cc6de59cfe3e4bf967e53e2ad36d11",
    "id": null,
    "metadata": {},
    "name": "LocationListEnqueueSyncMutation",
    "operationKind": "mutation",
    "text": "mutation LocationListEnqueueSyncMutation(\n  $locationId: ID!\n) {\n  enqueueMemberSync(locationId: $locationId)\n}\n"
  }
};
})();

(node as any).hash = "f7a5f24be2844444370d62c6d98882ed";

export default node;
