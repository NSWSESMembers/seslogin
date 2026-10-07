/**
 * @generated SignedSource<<85ad10a58718e63574305a72a501237a>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type ApiTokenNewMutation$variables = {
  expiresAt?: number | null | undefined;
  locationGrants: ReadonlyArray<string>;
  name: string;
  readOnly: boolean;
};
export type ApiTokenNewMutation$data = {
  readonly createApiToken: {
    readonly secret: string;
    readonly token: {
      readonly id: string;
      readonly name: string;
    };
  };
};
export type ApiTokenNewMutation = {
  response: ApiTokenNewMutation$data;
  variables: ApiTokenNewMutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "expiresAt"
},
v1 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "locationGrants"
},
v2 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "name"
},
v3 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "readOnly"
},
v4 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "expiresAt",
        "variableName": "expiresAt"
      },
      {
        "kind": "Variable",
        "name": "locationGrants",
        "variableName": "locationGrants"
      },
      {
        "kind": "Variable",
        "name": "name",
        "variableName": "name"
      },
      {
        "kind": "Variable",
        "name": "readOnly",
        "variableName": "readOnly"
      }
    ],
    "concreteType": "CreateApiTokenResult",
    "kind": "LinkedField",
    "name": "createApiToken",
    "plural": false,
    "selections": [
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "secret",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "concreteType": "ApiToken",
        "kind": "LinkedField",
        "name": "token",
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
    "name": "ApiTokenNewMutation",
    "selections": (v4/*:: as any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [
      (v2/*:: as any*/),
      (v1/*:: as any*/),
      (v3/*:: as any*/),
      (v0/*:: as any*/)
    ],
    "kind": "Operation",
    "name": "ApiTokenNewMutation",
    "selections": (v4/*:: as any*/)
  },
  "params": {
    "cacheID": "9ec49219ee36a12a67f27612af746a3f",
    "id": null,
    "metadata": {},
    "name": "ApiTokenNewMutation",
    "operationKind": "mutation",
    "text": "mutation ApiTokenNewMutation(\n  $name: String!\n  $locationGrants: [String!]!\n  $readOnly: Boolean!\n  $expiresAt: Int\n) {\n  createApiToken(name: $name, locationGrants: $locationGrants, readOnly: $readOnly, expiresAt: $expiresAt) {\n    secret\n    token {\n      id\n      name\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "667d40a6104f6e930b2ec689602c6a0e";

export default node;
