/**
 * @generated SignedSource<<9912d69e660137379dae115073deccc4>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type MembersListSyncMutation$variables = {
  locationId: string;
};
export type MembersListSyncMutation$data = {
  readonly enqueueMemberSync: boolean;
};
export type MembersListSyncMutation = {
  response: MembersListSyncMutation$data;
  variables: MembersListSyncMutation$variables;
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
    "name": "MembersListSyncMutation",
    "selections": (v1/*:: as any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Operation",
    "name": "MembersListSyncMutation",
    "selections": (v1/*:: as any*/)
  },
  "params": {
    "cacheID": "522aa6a77912bc73b5be035a100f04e1",
    "id": null,
    "metadata": {},
    "name": "MembersListSyncMutation",
    "operationKind": "mutation",
    "text": "mutation MembersListSyncMutation(\n  $locationId: ID!\n) {\n  enqueueMemberSync(locationId: $locationId)\n}\n"
  }
};
})();

(node as any).hash = "9d17d7f242293d668b9c3514103d1aab";

export default node;
