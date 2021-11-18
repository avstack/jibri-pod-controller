# jibri-pod-controller: A tool for managing the scaling of large Jibri deployments in Kubernetes.

When managing a large [Jibri](https://github.com/jitsi/jibri) deployment, you usually want to autoscale using a strategy like "always keep N spare (not recording or livestreaming) `jibri` pods". This is difficult to achieve with the standard Kubernetes horizontal pod autoscaler (HPA).

`jibri-pod-controller` can be used as part of an alternative approach:

 * Deploy `jibri` using a Deployment. Set the Deployment's `replicas` to the number of *spare* `jibri` pods you want to run. 
 * Deploy `jibri-pod-controller` in your cluster and give it RBAC permission to get/list/patch `jibri` pods.
 * Configure `jibri` in single use mode, and configure it to send webhook requests to `jibri-pod-controller`.
 * When a `jibri` pod starts to record or live-stream, `jibri-pod-controller` will patch the pod's labels so that they don't match the Deployment's label selector. This *isolates* the `jibri` pod from the Deployment — the Deployment will immediately launch another `jibri` pod to replace it (thus keeping the required number of spare pods), and the isolated `jibri` pod will continue to run.
 * When `jibri` finishes recording or live-streaming, `jibri-pod-controller` will delete the pod. A sweeper runs on a configurable interval to remove any expired `jibri` pods in case `jibri` fails to send the webhook for any reason. If multiple `jibri-pod-controller` pods are running, one is elected to run the sweeper.

## Building a container image from source

```
git clone https://github.com/avstack/jibri-pod-controller.git
cd jibri-pod-controller
docker build .
```

## Example

### `jibri.conf`

Irrelevant settings have been left out.

```
jibri {
  id = "$POD_NAME"
  single-use-mode = true

  api {
    http {
      // This must match the JIBRI_HEALTH_PORT environment variable in the jibri-pod-controller deployment
      external-api-port = 8080
    }
  }

  webhook {
    subscribers = [
      // $POD_NAME must be substituted with the Jibri pod name, for example by using envFrom to set it as
      // an environment variable and then using envsubst on jibri.conf in the container entrypoint script.
      "http://jibri-pod-controller.default.svc.cluster.local/webhook/$POD_NAME"
    ]
  }
}
```

### `jibri-pod-controller` deployment

* Replace the `image` with the URL to your built container image of `jibri-pod-controller`.
* In this example, the `JIBRI_BUSY_LABELS` are set to `app=jibri,state=busy`. You could set up your `jibri` Deployment to select `app=jibri,state=idle`.

```
apiVersion: rbac.authorization.k8s.io/v1
kind: Role
metadata:
  name: jibri-pod-controller
  namespace: default
rules:
- apiGroups:
  - ""
  resources:
  - pods
  verbs:
  - list
  - patch
  - get
  - delete
- apiGroups:
  - coordination.k8s.io
  resources:
  - leases
  verbs:
  - create
- apiGroups:
  - coordination.k8s.io
  resourceNames:
  - jibri-pod-controller-lease
  resources:
  - leases
  verbs:
  - update
  - patch
  - get

---

apiVersion: v1
kind: ServiceAccount
automountServiceAccountToken: true
metadata:
  name: jibri-pod-controller
  namespace: default

---

apiVersion: rbac.authorization.k8s.io/v1
kind: RoleBinding
metadata:
  name: jibri-pod-controller
  namespace: default
roleRef:
  apiGroup: rbac.authorization.k8s.io
  kind: Role
  name: jibri-pod-controller
subjects:
- kind: ServiceAccount
  name: jibri-pod-controller
  namespace: default

---

apiVersion: apps/v1
kind: Deployment
metadata:
  name: jibri-pod-controller
  namespace: default
spec:
  replicas: 2
  selector:
    matchLabels:
      app: jibri-pod-controller
  template:
    metadata:
      labels:
        app: jibri-pod-controller
    spec:
      automountServiceAccountToken: true
      serviceAccountName: jibri-pod-controller
      terminationGracePeriodSeconds: 30
      containers:
      - name: jibri-pod-controller
        image: your.image.registry.url/jibri-pod-controller:your-tag
        env:
        - name: RUST_LOG
          value: info
        - name: PORT
          value: "8080"
        - name: JIBRI_HEALTH_PORT
          value: "8080"
        - name: JIBRI_BUSY_LABELS
          value: app=jibri,state=busy
        - name: SWEEP_INTERVAL
          value: "300"
        - name: POD_NAME
          valueFrom:
            fieldRef:
              fieldPath: metadata.name
        - name: NAMESPACE
          valueFrom:
            fieldRef:
              fieldPath: metadata.namespace
        ports:
        - containerPort: 8080
          name: http
          protocol: TCP
        readinessProbe:
          failureThreshold: 1
          httpGet:
            path: /
            port: http
            scheme: HTTP
          initialDelaySeconds: 3
          periodSeconds: 10
          successThreshold: 1
          timeoutSeconds: 1
        resources:
          requests:
            cpu: 5m
            memory: 8Mi
        securityContext:
          allowPrivilegeEscalation: false
          privileged: false
          readOnlyRootFilesystem: true
      securityContext:
        runAsGroup: 1000
        runAsNonRoot: true
        runAsUser: 1000
      topologySpreadConstraints:
      - labelSelector:
          matchExpressions:
          - key: app
            operator: In
            values:
            - jibri-pod-controller
        maxSkew: 1
        topologyKey: kubernetes.io/hostname
        whenUnsatisfiable: ScheduleAnyway
      - labelSelector:
          matchExpressions:
          - key: app
            operator: In
            values:
            - jibri-pod-controller
        maxSkew: 1
        topologyKey: failure-domain.beta.kubernetes.io/zone
        whenUnsatisfiable: ScheduleAnyway

---

apiVersion: v1
kind: Service
metadata:
  name: jibri-pod-controller
  namespace: default
spec:
  type: ClusterIP
  selector:
    app: jibri-pod-controller
  ports:
  - name: http
    port: 80
    protocol: TCP
    targetPort: 8080
```

## License

`jibri-pod-controller` is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Any kinds of contributions are welcome as a pull request.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in these crates by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

## Acknowledgements

`jibri-pod-controller` development is sponsored by [AVStack](https://www.avstack.io/). We provide globally-distributed, scalable, managed Jitsi Meet backends.
