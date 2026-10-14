/**
 * @generated SignedSource<<9460b9cce9a6838a79cbecf53eb0e53f>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type LocationListToggleMutation$variables = {
  enabled: boolean;
  id: string;
  name: string;
  nitcEnabled?: number | null | undefined;
};
export type LocationListToggleMutation$data = {
  readonly updateLocation: {
    readonly enabled: boolean;
    readonly id: string;
    readonly name: string;
    readonly nitcEnabled: number | null | undefined;
  };
};
export type LocationListToggleMutation = {
  response: LocationListToggleMutation$data;
  variables: LocationListToggleMutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "enabled"
},
v1 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "id"
},
v2 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "name"
},
v3 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "nitcEnabled"
},
v4 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "enabled",
        "variableName": "enabled"
      },
      {
        "kind": "Variable",
        "name": "id",
        "variableName": "id"
      },
      {
        "kind": "Variable",
        "name": "name",
        "variableName": "name"
      },
      {
        "kind": "Variable",
        "name": "nitcEnabled",
        "variableName": "nitcEnabled"
      }
    ],
    "concreteType": "Location",
    "kind": "LinkedField",
    "name": "updateLocation",
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
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "enabled",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "nitcEnabled",
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
    "name": "LocationListToggleMutation",
    "selections": (v4/*:: as any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [
      (v1/*:: as any*/),
      (v2/*:: as any*/),
      (v0/*:: as any*/),
      (v3/*:: as any*/)
    ],
    "kind": "Operation",
    "name": "LocationListToggleMutation",
    "selections": (v4/*:: as any*/)
  },
  "params": {
    "cacheID": "1b1d71d9d6f7823cc5cf3dcffadda2be",
    "id": null,
    "metadata": {},
    "name": "LocationListToggleMutation",
    "operationKind": "mutation",
    "text": "mutation LocationListToggleMutation(\n  $id: ID!\n  $name: String!\n  $enabled: Boolean!\n  $nitcEnabled: Int\n) {\n  updateLocation(id: $id, name: $name, enabled: $enabled, nitcEnabled: $nitcEnabled) {\n    id\n    name\n    enabled\n    nitcEnabled\n  }\n}\n"
  }
};
})();

(node as any).hash = "6ce0e21af7c1cfb0642b69ce565e27c2";

export default node;
