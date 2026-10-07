/**
 * @generated SignedSource<<19d987c391e6fa36dd9ca8e0bf6abce2>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type NitcGroupNewMutation$variables = {
  id?: string | null | undefined;
  nitcTagIds: ReadonlyArray<number>;
  nitcType: string;
};
export type NitcGroupNewMutation$data = {
  readonly createNitcGroup: {
    readonly id: string;
    readonly nitcType: string;
  };
};
export type NitcGroupNewMutation = {
  response: NitcGroupNewMutation$data;
  variables: NitcGroupNewMutation$variables;
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
v3 = [
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
    "name": "createNitcGroup",
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
        "name": "nitcType",
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
    "name": "NitcGroupNewMutation",
    "selections": (v3/*:: as any*/),
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
    "name": "NitcGroupNewMutation",
    "selections": (v3/*:: as any*/)
  },
  "params": {
    "cacheID": "b124af1cfb60394f6c2c8df0efea2c7c",
    "id": null,
    "metadata": {},
    "name": "NitcGroupNewMutation",
    "operationKind": "mutation",
    "text": "mutation NitcGroupNewMutation(\n  $id: String\n  $nitcType: String!\n  $nitcTagIds: [Int!]!\n) {\n  createNitcGroup(id: $id, nitcType: $nitcType, nitcTagIds: $nitcTagIds) {\n    id\n    nitcType\n  }\n}\n"
  }
};
})();

(node as any).hash = "218c9f334cd32e00ed8859134203408a";

export default node;
