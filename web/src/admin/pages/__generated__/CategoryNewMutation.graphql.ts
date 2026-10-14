/**
 * @generated SignedSource<<88bb52b06c8dc1a4ed28b20d4b3a0e1c>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type CategoryNewMutation$variables = {
  isVirtual: boolean;
  name: string;
  nitcGroupId?: string | null | undefined;
  nitcParticipantType?: string | null | undefined;
};
export type CategoryNewMutation$data = {
  readonly createCategory: {
    readonly id: string;
    readonly name: string;
  };
};
export type CategoryNewMutation = {
  response: CategoryNewMutation$data;
  variables: CategoryNewMutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "isVirtual"
},
v1 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "name"
},
v2 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "nitcGroupId"
},
v3 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "nitcParticipantType"
},
v4 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "isVirtual",
        "variableName": "isVirtual"
      },
      {
        "kind": "Variable",
        "name": "name",
        "variableName": "name"
      },
      {
        "kind": "Variable",
        "name": "nitcGroupId",
        "variableName": "nitcGroupId"
      },
      {
        "kind": "Variable",
        "name": "nitcParticipantType",
        "variableName": "nitcParticipantType"
      }
    ],
    "concreteType": "Category",
    "kind": "LinkedField",
    "name": "createCategory",
    "plural": false,
    "selections": [
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "id",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "name",
        "storageKey": null
      }
    ],
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": [
      (v0/*:: as any*/),
      (v1/*:: as any*/),
      (v2/*:: as any*/),
      (v3/*:: as any*/)
    ],
    "kind": "Fragment",
    "metadata": null,
    "name": "CategoryNewMutation",
    "selections": (v4/*:: as any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [
      (v1/*:: as any*/),
      (v0/*:: as any*/),
      (v2/*:: as any*/),
      (v3/*:: as any*/)
    ],
    "kind": "Operation",
    "name": "CategoryNewMutation",
    "selections": (v4/*:: as any*/)
  },
  "params": {
    "cacheID": "23d50598e642e491eedf8d996d2435f5",
    "id": null,
    "metadata": {},
    "name": "CategoryNewMutation",
    "operationKind": "mutation",
    "text": "mutation CategoryNewMutation(\n  $name: String!\n  $isVirtual: Boolean!\n  $nitcGroupId: String\n  $nitcParticipantType: String\n) {\n  createCategory(name: $name, isVirtual: $isVirtual, nitcGroupId: $nitcGroupId, nitcParticipantType: $nitcParticipantType) {\n    id\n    name\n  }\n}\n"
  }
};
})();

(node as any).hash = "f57ab7548e4f179a2ef9b8a6bcf2587c";

export default node;
