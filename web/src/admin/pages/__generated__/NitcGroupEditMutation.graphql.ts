/**
 * @generated SignedSource<<3078418d86dfc893e40e678b623465b8>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type NitcGroupEditMutation$variables = {
  id: string;
  nitcTagIds: ReadonlyArray<number>;
  nitcType: string;
};
export type NitcGroupEditMutation$data = {
  readonly updateNitcGroup: {
    readonly id: string;
    readonly nitcType: string;
    readonly sesTags: ReadonlyArray<{
      readonly id: string;
      readonly name: string;
    }>;
  };
};
export type NitcGroupEditMutation = {
  response: NitcGroupEditMutation$data;
  variables: NitcGroupEditMutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "id"
},
v1 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "nitcTagIds"
},
v2 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "nitcType"
},
v3 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "id",
  "storageKey": null
},
v4 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "id",
        "variableName": "id"
      },
      {
        "kind": "Variable",
        "name": "nitcTagIds",
        "variableName": "nitcTagIds"
      },
      {
        "kind": "Variable",
        "name": "nitcType",
        "variableName": "nitcType"
      }
    ],
    "concreteType": "NitcGroup",
    "kind": "LinkedField",
    "name": "updateNitcGroup",
    "plural": false,
    "selections": [
      (v3/*:: as any*/),
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "nitcType",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "concreteType": "SesNonIncidentTag",
        "kind": "LinkedField",
        "name": "sesTags",
        "plural": true,
        "selections": [
          (v3/*:: as any*/),
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
    ],
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": [
      (v0/*:: as any*/),
      (v1/*:: as any*/),
      (v2/*:: as any*/)
    ],
    "kind": "Fragment",
    "metadata": null,
    "name": "NitcGroupEditMutation",
    "selections": (v4/*:: as any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [
      (v0/*:: as any*/),
      (v2/*:: as any*/),
      (v1/*:: as any*/)
    ],
    "kind": "Operation",
    "name": "NitcGroupEditMutation",
    "selections": (v4/*:: as any*/)
  },
  "params": {
    "cacheID": "3f612e3db2d6e4e41445ce2213e8f0c9",
    "id": null,
    "metadata": {},
    "name": "NitcGroupEditMutation",
    "operationKind": "mutation",
    "text": "mutation NitcGroupEditMutation(\n  $id: ID!\n  $nitcType: String!\n  $nitcTagIds: [Int!]!\n) {\n  updateNitcGroup(id: $id, nitcType: $nitcType, nitcTagIds: $nitcTagIds) {\n    id\n    nitcType\n    sesTags {\n      id\n      name\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "9276c81bdb4b30198ae3d6e014bcb7f5";

export default node;
